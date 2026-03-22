pub mod db;
pub mod file;
pub mod fold_rest;
pub mod note;
pub mod parse;

pub use db::DbValue;
pub use mlua::prelude::*;
pub use std::{fmt::Debug, path::Path};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MyError>;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),
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
        "get_notes",
        lua.create_function(|lua, (notes_dir, db_file): (String, String)| {
            lua_db(lua, db_file, notes_dir, |db| db.get_notes())
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
        "get_backlinks_2",
        lua.create_function( |lua, (content, col, row): (Vec<String>, usize, usize)| {
            let x = parse::parse_note(parse::LinesIter::List(content.into_iter())).into_lua_err()?.consume(None, None, None);

            Ok(LuaNil)
            // x.
            },
        )?,
    )?;
    exports.set(
        "get_path",
        lua.create_function(
            |lua, (notes_dir, db_file, id): (String, String, String)| {
                lua_db(lua, db_file, notes_dir, |db| db.get_path(id))
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
        "enter_command",
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
