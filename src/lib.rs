#![feature(trim_prefix_suffix)]
pub mod create;
pub mod db;
pub mod file;
pub mod fold_rest;
pub mod note;
pub mod parse;

pub use db::DbValue;
pub use mlua::prelude::*;
use std::time::Instant;
pub use std::{fmt::Debug, ops::Deref, path::Path};
use thiserror::Error;

use crate::note::RefNode;

pub struct Table(pub Vec<(String, LuaValue)>);

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

impl Deref for Table {
    type Target = Vec<(String, LuaValue)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type Result<T> = std::result::Result<T, MyError>;

pub trait ToNoteId {
    fn to_note_id(self) -> String;
}

impl ToNoteId for String {
    fn to_note_id(self) -> String {
        self.split(" ")
            .map(|x| {
                x.to_lowercase()
                    .chars()
                    .filter(|y| *y == '-' || y.is_ascii_alphanumeric())
                    .collect()
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
            lua_db(lua, db_file, notes_dir, |db| db.process_files())
        })?,
    )?;

    exports.set(
        "enter_command",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_tag_list())
        })?,
    )?;

    exports.set(
        "get_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_notes())
        })?,
    )?;

    exports.set(
        "get_thin_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_thin_notes())
        })?,
    )?;

    exports.set(
        "get_note_anchors",
        lua.create_function(|lua, (notes_dir, db_file, id): (String, String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_note_anchors(id))
        })?,
    )?;

    exports.set(
        "get_backlinks",
        lua.create_function(
            |lua, (notes_dir, db_file, note): (String, String, String)| {
                lua_db(lua, db_file, notes_dir, |db| db.get_backlinks(note))
            },
        )?,
    )?;

    exports.set(
        "id_gen",
        lua.create_function(|lua, title: String| Ok(title.to_note_id().into_lua(lua)))?,
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
                lua_db_multi(lua, &db_file, &notes_dir, |db| match note_type.as_str() {
                    "source" => {
                        let parent = db.get_frontmatter(&parent_id.ok_or(MyError::OtherError)?)?;
                        let (path, new_id) =
                            create::source(parent, &id, &title, &notes_dir, contents)?;

                        Ok((
                            "note",
                            Table(vec![
                                ("path".to_string(), path.into_lua(lua)?),
                                ("row".to_string(), 11.into_lua(lua)?),
                                (
                                    "replace".to_string(),
                                    format!("[[{new_id}|{title}]]").into_lua(lua)?,
                                ),
                            ]),
                        ))
                    }
                    "author" => {
                        let (path, _) = create::author(&id, &title, &notes_dir, contents)?;

                        Ok((
                            "note",
                            Table(vec![
                                ("path".to_string(), path.into_lua(lua)?),
                                ("row".to_string(), 11.into_lua(lua)?),
                            ]),
                        ))
                    }
                    "note" => {
                        let parent = db.get_frontmatter(&parent_id.ok_or(MyError::OtherError)?)?;
                        let parent_type = parent.note_type.clone(); 

                        let (path, new_id) =
                            create::note(parent, &id, &title, &notes_dir, contents)?;

                        if parent_type == "note" {
                            Ok((
                                "note",
                                Table(vec![
                                    ("path".to_string(), path.into_lua(lua)?),
                                    ("row".to_string(), 11.into_lua(lua)?),
                                    (
                                        "replace".to_string(),
                                        format!("[[{new_id}|{title}]]").into_lua(lua)?,
                                    ),
                                ]),
                            ))
                        } else {
                            Ok((
                                "note",
                                Table(vec![
                                    ("path".to_string(), path.into_lua(lua)?),
                                    ("row".to_string(), 11.into_lua(lua)?),
                                ]),
                            ))
                        }
                    },
                    _ => Err(MyError::OtherError)
                })
            },
        )?,
    )?;

    exports.set(
        "get_highlights",
        lua.create_function(|lua, content: Vec<String>| {
            let x = parse::parse_note(parse::LinesIter::List(content.into_iter()))
                .into_lua_err()?
                .consume(None, None, None);

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

            Ok(res)
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
                lua_db_multi(lua, &db_file, &notes_dir, |db| {
                    let start = Instant::now();
                    let x = parse::parse_note(parse::LinesIter::List(content.into_iter()))?
                        .consume(None, None, None);
                    let x = x.get_ref(col, row);

                    print!("Time Taken: {}s", start.elapsed().as_secs_f64());
                    Ok(match x {
                        Some(r) => match r.to {
                            RefNode::NoteAnchor(anchor) => {
                                let (path, row, col) = db.get_loc(anchor)?;
                                (
                                    "note",
                                    Table(vec![
                                        ("path".to_string(), path.into_lua(lua)?),
                                        ("row".to_string(), row.into_lua(lua)?),
                                        ("col".to_string(), col.into_lua(lua)?),
                                    ]),
                                )
                                    .into_lua_multi(lua)?
                            }
                            RefNode::Tag(tag) => ("tag", tag).into_lua_multi(lua)?,
                            _ => LuaNil.into_lua_multi(lua)?,
                        },
                        None => LuaNil.into_lua_multi(lua)?,
                    })
                })
            },
        )?,
    )?;

    exports.set(
        "get_tags",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_tag_list())
        })?,
    )?;

    exports.set(
        "get_tag_notes",
        lua.create_function(|lua, (notes_dir, db_file, tag): (String, String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_tag_notes(tag))
        })?,
    )?;

    Ok(exports)
}

pub fn lua_db<F, T: IntoLua>(
    lua: &Lua,
    db_path: String,
    notes_dir: String,
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

pub fn lua_db_multi<F, T: IntoLuaMulti>(
    lua: &Lua,
    db_path: &String,
    notes_dir: &String,
    f: F,
) -> LuaResult<LuaMultiValue>
where
    F: FnOnce(db::Db) -> Result<T>,
{
    db::Db::init(Path::new(&notes_dir), Path::new(&db_path))
        .and_then(f)
        .into_lua_err()
        .and_then(|x| x.into_lua_multi(lua))
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
