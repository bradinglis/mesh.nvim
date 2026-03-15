use duckdb::{
    Connection, Error, params,
    types::{FromSql, FromSqlError, OrderedMap},
};
use mlua::prelude::*;
use std::path::Path;
// const CACHE_FILE: &Path = Path::new("../.cache/mardown-db");

#[mlua::lua_module]
fn markdown_db(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set(
        "get_notes",
        lua.create_function(|lua, (): ()| get_notes(lua))?,
    )?;
    exports.set(
        "get_backlinks",
        lua.create_function(|lua, id: String | get_backlinks(lua, id))?,
    )?;
    Ok(exports)
}

#[derive(Debug,Clone)]
pub struct Notes {
    v: Vec<Note>,
}

#[derive(Debug,Clone)]
pub struct Note {
    id: Res,
    title: Res,
    authors: Res,
    tags: Res,
    links: Res,
}

#[derive(Debug,Clone)]
struct ListString {
    values: Vec<String>,
}

#[derive(Debug,Clone)]
enum Res {
    List(Vec<Res>),
    Table(Vec<(String, Res)>),
    String(String),
    None,
}

fn get_notes(lua: &Lua) -> LuaResult<LuaValue> {
    let x = db_get().into_lua_err()?;
    x.into_lua(lua)
}

fn get_backlinks(lua: &Lua, id: String) -> LuaResult<LuaValue> {
    let x = db_get_backlinks(id).into_lua_err()?;
    x.into_lua(lua)
}

impl IntoLua for Res {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        Ok(match self {
            Res::List(items) => {
                let t = lua.create_table()?;
                for (i, x) in items.into_iter().enumerate() {
                    if let Res::None = x {
                    } else {
                        t.set(i + 1, x)?
                    }
                }
                LuaValue::Table(t)
            }
            Res::Table(items) => {
                let t = lua.create_table()?;
                for (k, v) in items.into_iter() {
                    t.set(k, v)?
                }
                LuaValue::Table(t)
            }
            Res::String(x) => x.into_lua(lua)?,
            Res::None => LuaValue::Nil,
        })
    }
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

impl Res {
    fn from_value(value: duckdb::types::Value) -> duckdb::types::FromSqlResult<Self> {
        Ok(match value {
            duckdb::types::Value::Text(x) | duckdb::types::Value::Enum(x) => Res::String(x),
            duckdb::types::Value::List(values) | duckdb::types::Value::Array(values) => Res::List(
                values
                    .into_iter()
                    .filter_map(|v| Res::from_value(v).ok())
                    .collect(),
            ),
            duckdb::types::Value::Struct(ordered_map) => Res::Table({
                ordered_map
                    .iter()
                    .filter_map(|(x, y)| Some((x.to_owned(), Res::from_value(y.to_owned()).ok()?)))
                    .collect()
            }),
            _ => Res::None,
        })
    }
}

impl FromSql for Res {
    fn column_result(value: duckdb::types::ValueRef<'_>) -> duckdb::types::FromSqlResult<Self> {
        Res::from_value(value.to_owned())
    }
}

pub fn db_get() -> Result<Notes, duckdb::Error> {
    let conn = Connection::open("/home/brad/.cache/markdown-db")?;
    let mut stmt = conn
        .prepare(r#"select
notes.note_id,
notes.note_title,
list(distinct { 'id': tags.tag, 'line': tags.line }) as tags,
list(distinct { 'id': author_notes.note_id, 'name': author_notes.note_title }) as authors,
list(distinct { 'id': refs.to_id, 'line': refs.line}) as links,
from notes 
left join tags on notes.note_id = tags.note_id
left join authors on notes.note_id = authors.source
left join notes as author_notes on authors.author = author_notes.note_id
left join refs on notes.note_id = refs.note_id
full join parents on notes.note_id = parents.parent
group by notes.note_title,notes.note_id;
"#)?;

    let rows = stmt.query_map([], |x| {
        Ok(Note {
            id: x.get(0)?,
            title: x.get(1)?,
            tags: x.get(2)?,
            authors: x.get(3)?,
            links: x.get(4)?,
        })
    })?;

    let x: Vec<Note> = rows.map(|x| x.expect("Failed to map")).collect();

    Ok(Notes { v: x })
}


pub fn db_get_backlinks(id: String) -> Result<Notes, duckdb::Error> {
    let conn = Connection::open("/home/brad/.cache/markdown-db")?;
    let mut stmt = conn
        .prepare(r#"
select
notes.note_id,
notes.note_title,
list(distinct { 'id': tags.tag, 'line': tags.line }) as tags,
list(distinct { 'id': author_notes.note_id, 'name': author_notes.note_title }) as authors,
list(distinct { 'id': refs.to_id, 'line': refs.line}) as links,
from notes
left join tags on notes.note_id = tags.note_id
left join authors on notes.note_id = authors.source
left join notes as author_notes on authors.author = author_notes.note_id
left join refs on notes.note_id = refs.note_id
full join parents on notes.note_id = parents.parent
where notes.note_id in (select note_id from refs where to_id = ?)
group by notes.note_title,notes.note_id;
"#)?;

    let rows = stmt.query_map([id], |x| {
        Ok(Note {
            id: x.get(0)?,
            title: x.get(1)?,
            tags: x.get(2)?,
            authors: x.get(3)?,
            links: x.get(4)?,
        })
    })?;

    let x: Vec<Note> = rows.map(|x| x.expect("Failed to map")).collect();

    Ok(Notes { v: x })
}
