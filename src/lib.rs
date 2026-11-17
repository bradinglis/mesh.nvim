pub mod annotations;
pub mod api;
pub mod chunking;
pub mod cli;
pub mod create;
pub mod db;
pub mod embed_index;
pub mod embed_store;
pub mod embeddings;
pub mod file;
pub mod fold_rest;
pub mod note;
pub mod parse;
pub mod results;
pub mod semantic_search;
pub mod trier;

pub use db::DbValue;
use mlua::Function;
pub use mlua::prelude::*;
pub use std::{ops::Deref, path::Path};
use thiserror::Error;

use crate::annotations::display_outline_item;
use crate::note::{NoteType, RefNode};

pub struct Table(pub Vec<(String, LuaValue)>);
pub struct ReturnValue(pub Vec<(String, LuaValue)>);
pub struct NoteId(String);

impl IntoLua for Table {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        for (key, value) in self.0 {
            table.set(key, value)?;
        }
        Ok(LuaValue::Table(table))
    }
}

impl IntoLua for ReturnValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let result = lua.create_table()?;
        for (index, (run, opts)) in self.0.into_iter().enumerate() {
            let entry = lua.create_table()?;
            entry.set("run", run)?;
            entry.set("opts", opts)?;
            result.set(index + 1, entry)?;
        }
        Ok(LuaValue::Table(result))
    }
}

impl Deref for Table {
    type Target = Vec<(String, LuaValue)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for ReturnValue {
    type Target = Vec<(String, LuaValue)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl NoteId {
    fn derive_subheading(value: &str) -> Self {
        Self(derive_slug(value, false))
    }

    fn derive(value: &str) -> Self {
        Self(derive_slug(value, true))
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Turn a title into a stable slug.
///
/// Note ids deliberately drop leading-lowercase words, while subheading ids
/// keep every word.
fn derive_slug(value: &str, require_uppercase: bool) -> String {
    value
        .split_whitespace()
        .filter(|word| !require_uppercase || !word.starts_with(char::is_lowercase))
        .map(|word| {
            word.to_lowercase()
                .chars()
                .filter(|ch| *ch == '-' || ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("-")
}

pub type Results<T> = results::Results<T, MyError>;
pub type Result<T> = std::result::Result<T, MyError>;

pub trait ToNoteId {
    fn to_note_id(self) -> String;
}

impl ToNoteId for &str {
    fn to_note_id(self) -> String {
        derive_slug(self, true)
    }
}

impl ToNoteId for String {
    fn to_note_id(self) -> String {
        derive_slug(&self, true)
    }
}

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),
    #[error("MLua error: {0}")]
    Mlua(#[from] mlua::Error),
    #[error("Pdf error: {0}")]
    Pdf(#[from] pdf_oxide::Error),
    #[error("Message: {0}")]
    String(String),
    #[error("File error: {0}")]
    FileError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("A custom error occurred")]
    OtherError,
}

pub trait MyResult {
    fn i(self) -> Self;
}

impl<T> MyResult for Result<T> {
    fn i(self) -> Self {
        self.map_err(|error| {
            eprintln!("Error: {error}");
            error
        })
    }
}

fn highlight(
    lua: &Lua,
    group: &str,
    (line, start_col, end_col): (usize, usize, usize),
) -> LuaResult<LuaValue> {
    Table(vec![
        ("hl".to_string(), group.into_lua(lua)?),
        ("line".to_string(), line.into_lua(lua)?),
        ("start_col".to_string(), start_col.into_lua(lua)?),
        ("end_col".to_string(), end_col.into_lua(lua)?),
    ])
    .into_lua(lua)
}

fn open_action(lua: &Lua, path: String, replace: Option<String>) -> Result<ReturnValue> {
    let mut fields = vec![
        ("path".to_string(), path.into_lua(lua)?),
        ("row".to_string(), 11.into_lua(lua)?),
    ];
    if let Some(replace) = replace {
        fields.push(("replace".to_string(), replace.into_lua(lua)?));
    }

    Ok(ReturnValue(vec![(
        "open".to_string(),
        Table(fields).into_lua(lua)?,
    )]))
}

fn parse_search_opts(table: Option<LuaTable>) -> LuaResult<semantic_search::SearchOpts> {
    let mut opts = semantic_search::SearchOpts::default();
    let Some(table) = table else {
        return Ok(opts);
    };
    if let Some(limit) = table.get::<Option<usize>>("limit")? {
        opts.limit = limit;
    }
    if let Some(group) = table.get::<Option<String>>("group")? {
        opts.group = semantic_search::Group::parse(&group)
            .ok_or_else(|| LuaError::RuntimeError(format!("unknown group '{group}'")))?;
    }
    if let Some(exclude) = table.get::<Option<Vec<String>>>("exclude_ids")? {
        opts.exclude_ids = exclude;
    }
    if let Some(graph_seeds) = table.get::<Option<bool>>("graph_seeds")? {
        opts.graph_seeds = graph_seeds;
    }
    if let Some(refine_line) = table.get::<Option<bool>>("refine_line")? {
        opts.refine_line = refine_line;
    }
    if let Some(max_per_note) = table.get::<Option<usize>>("max_per_note")? {
        opts.max_per_note = max_per_note;
    }
    if let Some(max_per_author) = table.get::<Option<usize>>("max_per_author")? {
        opts.max_per_author = max_per_author;
    }
    if let Some(high_score) = table.get::<Option<f32>>("high_score")? {
        opts.high_score = high_score;
    }
    if let Some(filters) = table.get::<Option<LuaTable>>("filters")? {
        opts.types = filters.get::<Option<Vec<String>>>("type")?;
        if let Some(tags) = filters.get::<Option<Vec<String>>>("tags")? {
            opts.tags = tags;
        }
    }
    if let Some(weights) = table.get::<Option<LuaTable>>("weights")? {
        if let Some(value) = weights.get::<Option<f32>>("semantic")? {
            opts.weights.semantic = Some(value);
        }
        if let Some(value) = weights.get::<Option<f32>>("keyword")? {
            opts.weights.keyword = Some(value);
        }
        if let Some(value) = weights.get::<Option<f32>>("graph")? {
            opts.weights.graph = Some(value);
        }
    }
    Ok(opts)
}

#[mlua::lua_module]
fn mesh(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;

    exports.set(
        "update_index",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            let message = match api::update(&notes_dir, &db_file) {
                Ok(text) => ("log".to_string(), text.into_lua(lua)?),
                Err(error) => ("err".to_string(), error.to_string().into_lua(lua)?),
            };
            Ok(ReturnValue(vec![message]))
        })?,
    )?;

    exports.set(
        "enter_command",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::tags(&db))
        })?,
    )?;

    exports.set(
        "get_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::notes(&db))
        })?,
    )?;

    exports.set(
        "get_lines",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::lines(&db))
        })?,
    )?;

    exports.set(
        "get_type",
        lua.create_function(
            |lua, (notes_dir, db_file, note_type): (String, String, String)| {
                lua_db(lua, &db_file, &notes_dir, |db| {
                    api::type_notes(&db, &note_type)
                })
            },
        )?,
    )?;

    exports.set(
        "get_thin_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::thin_notes(&db))
        })?,
    )?;

    exports.set(
        "get_annotations",
        lua.create_function(|lua, (pdf, note_path): (String, String)| {
            let pdf_path = Path::new(&pdf);
            let an_res = annotations::get_annotations(pdf_path).into_lua_err()?;

            let output = display_outline_item(&an_res, 1);
            let to_append: Vec<String> = output.split('\n').map(String::from).collect();

            Ok(ReturnValue(vec![(
                "append".to_string(),
                Table(vec![
                    ("contents".to_string(), to_append.into_lua(lua)?),
                    ("path".to_string(), note_path.into_lua(lua)?),
                ])
                .into_lua(lua)?,
            )]))
        })?,
    )?;

    exports.set(
        "get_note_anchors",
        lua.create_function(|lua, (notes_dir, db_file, id): (String, String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::note_anchors(&db, &id))
        })?,
    )?;

    exports.set(
        "get_backlinks",
        lua.create_function(
            |lua, (notes_dir, db_file, note): (String, String, String)| {
                lua_db(lua, &db_file, &notes_dir, |db| api::backlinks(&db, &note))
            },
        )?,
    )?;

    exports.set(
        "id_gen",
        lua.create_function(|lua, title: String| Ok(api::id_gen(&title).into_lua(lua)))?,
    )?;

    exports.set(
        "create_new",
        lua.create_function(
            |lua,
             (notes_dir, db_file, note_type, title, id, parent_id, contents): (
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<Vec<String>>,
            )| {
                lua_db(
                    lua,
                    &db_file,
                    &notes_dir,
                    |db| match NoteType::from_string(&note_type) {
                        NoteType::Source => {
                            let parent = db.get_frontmatter(
                                parent_id.as_deref().ok_or(MyError::OtherError)?,
                            )?;
                            let (path, new_id) =
                                create::source(parent, &id, &title, &notes_dir, contents)?;

                            open_action(lua, path, Some(format!("[[{new_id}|{title}]]")))
                        }
                        NoteType::Author => {
                            let (path, _) = create::author(&id, &title, &notes_dir, contents)?;
                            open_action(lua, path, None)
                        }
                        NoteType::Note => {
                            let parent = db.get_frontmatter(
                                parent_id.as_deref().ok_or(MyError::OtherError)?,
                            )?;
                            let parent_is_note = parent.note_type == "note";
                            let (path, new_id) =
                                create::note(parent, &id, &title, &notes_dir, contents)?;
                            let replace = parent_is_note.then(|| format!("[[{new_id}|{title}]]"));

                            open_action(lua, path, replace)
                        }
                        NoteType::Journal => {
                            let (path, _) = create::journal(&notes_dir, contents)?;
                            open_action(lua, path, None)
                        }
                        NoteType::None => Err(MyError::OtherError),
                    },
                )
            },
        )?,
    )?;

    exports.set(
        "get_highlights",
        lua.create_function(|lua, content: Vec<String>| {
            let parsed =
                parse::parse_note(parse::LinesIter::List(content.into_iter())).into_lua_err()?;

            let mut highlights: Vec<LuaValue> = Vec::new();

            for reference in parsed
                .links
                .iter()
                .chain(&parsed.author)
                .chain(&parsed.source_parents)
            {
                if let note::RefLoc::FrontMatter(line, start_col, end_col) = reference.loc {
                    highlights.push(highlight(
                        lua,
                        "@markup.link.label",
                        (line, start_col, end_col),
                    )?);
                }
            }

            for tag in &parsed.tags {
                highlights.push(highlight(lua, "ObsidianTag", tag.loc.get_loc())?);
            }

            Ok(ReturnValue(vec![(
                "highlight".to_string(),
                highlights.into_lua(lua)?,
            )]))
        })?,
    )?;
    exports.set(
        "cursor_action",
        lua.create_function(
            |lua,
             (notes_dir, db_file, content, line, col): (
                String,
                String,
                Vec<String>,
                usize,
                usize,
            )| {
                lua_db(lua, &db_file, &notes_dir, |db| {
                    let parsed = parse::parse_note(parse::LinesIter::List(content.into_iter()))?;

                    match parsed.get_ref(line, col) {
                        Some(reference) => match reference.to {
                            RefNode::NoteAnchor(anchor) => {
                                let (path, row, col) = db.get_loc(anchor)?;
                                Ok(ReturnValue(vec![(
                                    "open".to_string(),
                                    Table(vec![
                                        ("path".to_string(), path.into_lua(lua)?),
                                        ("row".to_string(), row.into_lua(lua)?),
                                        ("col".to_string(), col.into_lua(lua)?),
                                    ])
                                    .into_lua(lua)?,
                                )]))
                            }
                            RefNode::Tag(tag) => {
                                Ok(ReturnValue(vec![("tag".to_string(), tag.into_lua(lua)?)]))
                            }
                            _ => Ok(ReturnValue(vec![])),
                        },
                        None => Ok(ReturnValue(vec![])),
                    }
                })
            },
        )?,
    )?;

    exports.set(
        "get_tags",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::tags(&db))
        })?,
    )?;

    exports.set(
        "get_tag_notes",
        lua.create_function(|lua, (notes_dir, db_file, tag): (String, String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| api::tag_notes(&db, &tag))
        })?,
    )?;

    exports.set(
        "update_embeddings",
        lua.create_function(
            |lua, (notes_dir, db_file, progress): (String, String, Option<Function>)| {
                let mut report = |done: usize, total: usize| {
                    if let Some(callback) = &progress {
                        let _ = callback.call::<()>((done, total));
                    }
                };
                let message = match api::embed_refresh(&notes_dir, &db_file, &mut report) {
                    Ok(text) => ("log".to_string(), text.into_lua(lua)?),
                    Err(error) => ("err".to_string(), error.to_string().into_lua(lua)?),
                };
                Ok(ReturnValue(vec![message]))
            },
        )?,
    )?;

    exports.set(
        "warm_model",
        lua.create_function(|lua, db_file: String| {
            let message = match api::warm_model(&db_file) {
                Ok(text) => ("log".to_string(), text.into_lua(lua)?),
                Err(error) => ("err".to_string(), error.to_string().into_lua(lua)?),
            };
            Ok(ReturnValue(vec![message]))
        })?,
    )?;

    exports.set(
        "semantic_search_prompt",
        lua.create_function(
            |lua,
             (notes_dir, db_file, prompt, context_note, opts): (
                String,
                String,
                String,
                Option<String>,
                Option<LuaTable>,
            )| {
                let opts = parse_search_opts(opts)?;
                lua_db(lua, &db_file, &notes_dir, |db| {
                    api::search_prompt(&db, &db_file, &prompt, context_note, &opts)
                })
            },
        )?,
    )?;

    exports.set(
        "semantic_search_note",
        lua.create_function(
            |lua, (notes_dir, db_file, note_id, opts): (String, String, String, Option<LuaTable>)| {
                let opts = parse_search_opts(opts)?;
                lua_db(lua, &db_file, &notes_dir, |db| {
                    api::search_note(&db, &db_file, &note_id, &opts)
                })
            },
        )?,
    )?;

    Ok(exports)
}

pub fn lua_db<F, T: IntoLua>(lua: &Lua, db_path: &str, notes_dir: &str, f: F) -> LuaResult<LuaValue>
where
    F: FnOnce(db::Db) -> Result<T>,
{
    db::Db::init(Path::new(notes_dir), Path::new(db_path))
        .and_then(f)
        .into_lua_err()
        .and_then(|x| x.into_lua(lua))
}

/// Drop `None` entries so Lua receives a dense sequence.
fn compact_list(items: Vec<DbValue>) -> Vec<DbValue> {
    items
        .into_iter()
        .filter(|item| !matches!(item, DbValue::None))
        .collect()
}

impl IntoLua for db::DbValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        Ok(match self {
            DbValue::List(items) => {
                let t = lua.create_table()?;
                for (i, x) in compact_list(items).into_iter().enumerate() {
                    t.set(i + 1, x)?
                }
                LuaValue::Table(t)
            }
            DbValue::Table(items) => {
                let t = lua.create_table()?;
                for (k, v) in items.into_iter() {
                    t.set(k, v)?
                }
                LuaValue::Table(t)
            }
            DbValue::String(x) => x.into_lua(lua)?,
            DbValue::Int(x) => LuaValue::Integer(x),
            DbValue::Float(x) => LuaValue::Number(x),
            DbValue::Bool(x) => LuaValue::Boolean(x),
            DbValue::None => LuaValue::Nil,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_id_drops_lowercase_words() {
        assert_eq!("Foo Bar".to_string().to_note_id(), "foo-bar");
        assert_eq!("foo Bar".to_string().to_note_id(), "bar");
        assert_eq!("ALL CAPS".to_string().to_note_id(), "all-caps");
    }

    #[test]
    fn compact_list_removes_nulls_without_holes() {
        let items = vec![
            DbValue::String("a".to_string()),
            DbValue::None,
            DbValue::String("b".to_string()),
        ];
        let compacted = compact_list(items);
        assert_eq!(compacted.len(), 2);
        assert!(matches!(&compacted[1], DbValue::String(s) if s == "b"));
    }
}
