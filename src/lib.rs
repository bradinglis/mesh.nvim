#![feature(trim_prefix_suffix)]
#![feature(str_from_utf16_endian)]
#![feature(try_trait_v2)]
#![feature(iter_intersperse)]
#![feature(string_from_utf8_lossy_owned)]

pub mod annotations;
pub mod create;
pub mod db;
pub mod file;
pub mod fold_rest;
pub mod note;
pub mod parse;
pub mod results;
pub mod trier;

pub use db::DbValue;
pub use mlua::prelude::*;
pub use std::{fmt::Debug, ops::Deref, path::Path};
use std::{
    string::{FromUtf8Error, FromUtf16Error},
    time::Instant,
};
use thiserror::Error;

use crate::note::RefNode;

pub struct Table(pub Vec<(String, LuaValue)>);
pub struct ReturnValue(pub Vec<(String, LuaValue)>);
pub struct NoteId(String);

impl IntoLua for Table {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let items = self.to_owned();

        let t = lua.create_table()?;
        for (k, v) in items.into_iter() {
            t.set(k, v)?
        }

        Ok(LuaValue::Table(t))
    }
}

impl IntoLua for ReturnValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let items = self.to_owned();
        let r = lua.create_table()?;
        for (k, (s, t)) in items.into_iter().enumerate() {
            let st = lua.create_table()?;
            st.set("run", s)?;
            st.set("opts", t)?;
            r.set(k + 1, st)?;
        }
        Ok(LuaValue::Table(r))
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

impl Deref for NoteId {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl NoteId {
    fn derive(value: &str) -> Self {
        NoteId(
            value
                .split_whitespace()
                .filter_map(|word| {
                    if word.starts_with(|ch: char| ch.is_lowercase()) {
                        None
                    } else {
                        Some(
                            word.to_lowercase()
                                .chars()
                                .filter(|y| *y == '-' || y.is_ascii_alphanumeric())
                                .collect(),
                        )
                    }
                })
                .collect::<Vec<String>>()
                .join("-"),
        )
    }
}

pub type Results<T> = results::Results<T, MyError>;
pub type Result<T> = std::result::Result<T, MyError>;

pub trait ToNoteId {
    fn to_note_id(self) -> String;
}

impl ToNoteId for String {
    fn to_note_id(self) -> String {
        self.split_whitespace()
            .filter_map(|word| {
                if word.starts_with(|ch: char| ch.is_lowercase()) {
                    None
                } else {
                    Some(
                        word.to_lowercase()
                            .chars()
                            .filter(|y| *y == '-' || y.is_ascii_alphanumeric())
                            .collect(),
                    )
                }
            })
            .collect::<Vec<String>>()
            .join("-")
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
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Pdf error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("Utf16 error: {0}")]
    Utf16(#[from] FromUtf16Error),
    #[error("Utf8 error: {0}")]
    Utf8(#[from] FromUtf8Error),
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
        match self {
            Err(y) => {
                eprintln!("Error: {}", y);
                Err(y)
            }
            x => x,
        }
    }
}

#[mlua::lua_module]
fn markdown_db(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;

    exports.set(
        "update_index",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| {
                Ok(ReturnValue(vec![
                    ("log".to_string(), db.process_files()?.into_lua(lua)?),
                    // ("err".to_string(), "whoops".into_lua(lua)?),
                ]))
            })
        })?,
    )?;

    exports.set(
        "enter_command",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| db.get_tag_list())
        })?,
    )?;

    exports.set(
        "get_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| db.get_notes())
        })?,
    )?;

    exports.set(
        "get_lines",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| db.get_lines())
        })?,
    )?;

    exports.set(
        "get_type",
        lua.create_function(
            |lua, (notes_dir, db_file, note_type): (String, String, String)| {
                lua_db(lua, &db_file, &notes_dir, |db| db.get_type_notes(note_type))
            },
        )?,
    )?;

    exports.set(
        "get_thin_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| db.get_thin_notes())
        })?,
    )?;

    exports.set(
        "get_annotations",
        lua.create_function(|lua, (pdf, note_path): (String, String)| {
            let pdf_path = Path::new(&pdf);
            let an_res = annotations::get_annotations(pdf_path);

            let mut to_append: Vec<String> = Vec::new();
            to_append.push("## Annotations Export".to_string());
            to_append.push("".to_string());

            for annotation in an_res.oks.into_iter() {
                if let Some(comment) = annotation.comment {
                    if !to_append.last().map_or("", |s| s).is_empty() {
                        to_append.push("".to_string());
                    }
                    // let c: Vec<String> = comment.split("\n").map(String::from).collect();
                    to_append.append(&mut comment.split("\n").map(String::from).collect());
                }

                let highlighted: String = annotation
                    .quote
                    .split_whitespace()
                    .map(str::trim)
                    .intersperse(" ")
                    .collect();

                match annotation.priority {
                    annotations::Priority::High => {
                        if !to_append.last().map_or("", |s| s).is_empty() {
                            to_append.push("".to_string());
                        }
                        to_append.push(format!(
                            "> [!QUOTE] {} {{p. {}}}",
                            highlighted, annotation.page
                        ));
                        to_append.push("".to_string());
                    }
                    annotations::Priority::Medium | annotations::Priority::Low => {
                        to_append.push(format!("- {} {{p. {}}}", highlighted, annotation.page));
                    }
                };
            }

            let mut res: Vec<(String, mlua::Value)> = an_res
                .errs
                .into_iter()
                .map(|err| ("err".to_string(), err.to_string().into_lua(lua).unwrap()))
                .collect();

            res.push((
                "append".to_string(),
                Table(vec![
                    ("contents".to_string(), to_append.into_lua(lua)?),
                    ("path".to_string(), note_path.into_lua(lua)?),
                ])
                .into_lua(lua)?,
            ));

            Ok(ReturnValue(res))
        })?,
    )?;

    exports.set(
        "get_note_anchors",
        lua.create_function(|lua, (notes_dir, db_file, id): (String, String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| db.get_note_anchors(id))
        })?,
    )?;

    exports.set(
        "get_backlinks",
        lua.create_function(
            |lua, (notes_dir, db_file, note): (String, String, String)| {
                lua_db(lua, &db_file, &notes_dir, |db| db.get_backlinks(note))
            },
        )?,
    )?;

    exports.set(
        "id_gen",
        lua.create_function(|lua, title: String| {
            Ok(NoteId::derive(&title).to_owned().into_lua(lua))
        })?,
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
                lua_db(lua, &db_file, &notes_dir, |db| match note_type.as_str() {
                    "source" => {
                        let parent = db.get_frontmatter(&parent_id.ok_or(MyError::OtherError)?)?;
                        let (path, new_id) =
                            create::source(parent, &id, &title, &notes_dir, contents)?;

                        Ok(ReturnValue(vec![(
                            "open".to_string(),
                            Table(vec![
                                ("path".to_string(), path.into_lua(lua)?),
                                ("row".to_string(), 11.into_lua(lua)?),
                                (
                                    "replace".to_string(),
                                    format!("[[{new_id}|{title}]]").into_lua(lua)?,
                                ),
                            ])
                            .into_lua(lua)?,
                        )]))
                    }
                    "author" => {
                        let (path, _) = create::author(&id, &title, &notes_dir, contents)?;

                        Ok(ReturnValue(vec![(
                            "open".to_string(),
                            Table(vec![
                                ("path".to_string(), path.into_lua(lua)?),
                                ("row".to_string(), 11.into_lua(lua)?),
                            ])
                            .into_lua(lua)?,
                        )]))
                    }
                    "note" => {
                        let parent = db.get_frontmatter(&parent_id.ok_or(MyError::OtherError)?)?;
                        let parent_type = parent.note_type.clone();

                        let (path, new_id) =
                            create::note(parent, &id, &title, &notes_dir, contents)?;

                        if parent_type == "note" {
                            Ok(ReturnValue(vec![(
                                "open".to_string(),
                                Table(vec![
                                    ("path".to_string(), path.into_lua(lua)?),
                                    ("row".to_string(), 11.into_lua(lua)?),
                                    (
                                        "replace".to_string(),
                                        format!("[[{new_id}|{title}]]").into_lua(lua)?,
                                    ),
                                ])
                                .into_lua(lua)?,
                            )]))
                        } else {
                            Ok(ReturnValue(vec![(
                                "open".to_string(),
                                Table(vec![
                                    ("path".to_string(), path.into_lua(lua)?),
                                    ("row".to_string(), 11.into_lua(lua)?),
                                ])
                                .into_lua(lua)?,
                            )]))
                        }
                    }
                    "journal" => {
                        let (path, _) = create::journal(&notes_dir, contents)?;

                        Ok(ReturnValue(vec![(
                            "open".to_string(),
                            Table(vec![
                                ("path".to_string(), path.into_lua(lua)?),
                                ("row".to_string(), 11.into_lua(lua)?),
                            ])
                            .into_lua(lua)?,
                        )]))
                    }
                    _ => Err(MyError::OtherError),
                })
            },
        )?,
    )?;

    exports.set(
        "get_highlights",
        lua.create_function(|lua, content: Vec<String>| {
            let x =
                parse::parse_note(parse::LinesIter::List(content.into_iter())).into_lua_err()?;

            let mut res: Vec<LuaValue> = vec![];

            let y = [x.links.clone(), x.author.clone(), x.source_parents.clone()].concat();

            let frontmatter_refs: Vec<(usize, usize, usize)> = y
                .into_iter()
                .filter_map(|x| {
                    if let note::RefLoc::FrontMatter(x, y, z) = x.loc {
                        Some((x, y, z))
                    } else {
                        None
                    }
                })
                .collect();

            for (line, start_col, end_col) in frontmatter_refs {
                res.push(
                    Table(vec![
                        ("hl".to_string(), "@markup.link.label".into_lua(lua)?),
                        ("line".to_string(), line.into_lua(lua)?),
                        ("start_col".to_string(), start_col.into_lua(lua)?),
                        ("end_col".to_string(), end_col.into_lua(lua)?),
                    ])
                    .into_lua(lua)?,
                )
            }

            let j: Vec<(usize, usize, usize)> =
                x.tags.into_iter().map(|t| t.loc.get_loc()).collect();

            for (line, start_col, end_col) in j {
                res.push(
                    Table(vec![
                        ("hl".to_string(), "ObsidianTag".into_lua(lua)?),
                        ("line".to_string(), line.into_lua(lua)?),
                        ("start_col".to_string(), start_col.into_lua(lua)?),
                        ("end_col".to_string(), end_col.into_lua(lua)?),
                    ])
                    .into_lua(lua)?,
                )
            }

            Ok(ReturnValue(vec![(
                "highlight".to_string(),
                res.into_lua(lua)?,
            )]))
        })?,
    )?;
    exports.set(
        "cursor_action",
        lua.create_function(
            |lua,
             (notes_dir, db_file, content, col, row): (
                String,
                String,
                Vec<String>,
                usize,
                usize,
            )| {
                lua_db(lua, &db_file, &notes_dir, |db| {
                    let start = Instant::now();
                    let x = parse::parse_note(parse::LinesIter::List(content.into_iter()))?;
                    let x = x.get_ref(col, row);

                    print!("Time Taken: {}s", start.elapsed().as_secs_f64());
                    match x {
                        Some(r) => match r.to {
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
            lua_db(lua, &db_file, &notes_dir, |db| db.get_tag_list())
        })?,
    )?;

    exports.set(
        "get_tag_notes",
        lua.create_function(|lua, (notes_dir, db_file, tag): (String, String, String)| {
            lua_db(lua, &db_file, &notes_dir, |db| db.get_tag_notes(tag))
        })?,
    )?;

    Ok(exports)
}

pub fn lua_db<F, T: IntoLua>(
    lua: &Lua,
    db_path: &String,
    notes_dir: &String,
    f: F,
) -> LuaResult<LuaValue>
where
    F: FnOnce(db::Db) -> Result<T>,
{
    db::Db::init(Path::new(&notes_dir), Path::new(&db_path))
        .and_then(f)
        .into_lua_err()
        .and_then(|x| x.into_lua(lua))
}

pub fn bin_do<F, T: Debug>(db_path: String, notes_dir: String, f: F) -> Result<T>
where
    F: FnOnce(db::Db) -> Result<T>,
{
    db::Db::init(Path::new(&notes_dir), Path::new(&db_path)).and_then(f)
}

impl IntoLua for db::DbValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        Ok(match self {
            DbValue::List(items) => {
                let t = lua.create_table()?;
                for (i, x) in items.into_iter().enumerate() {
                    if let DbValue::None = x {
                    } else {
                        t.set(i + 1, x)?
                    }
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

// May use for picker simplicity
#[derive(Debug, Clone)]
pub struct Notes {
    v: Vec<Note>,
}

#[derive(Debug, Clone)]
pub struct Note {
    id: DbValue,
    title: DbValue,
    authors: DbValue,
    tags: DbValue,
    links: DbValue,
}

impl IntoLua for Notes {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let t = lua.create_table()?;
        for x in self.v.into_iter() {
            t.set(x.id.clone(), x)?
        }
        Ok(LuaValue::Table(t))
    }
}

impl IntoLua for Note {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let t = lua.create_table()?;
        t.set("id", self.id)?;
        t.set("title", self.title)?;
        t.set("tags", self.tags)?;
        t.set("authors", self.authors)?;
        t.set("links", self.links)?;
        Ok(LuaValue::Table(t))
    }
}
