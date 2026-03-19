use std::{collections::HashMap, path::PathBuf, time::SystemTime};

use duckdb::{Params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::{MyError, Result, file::Paths, note};

#[derive(Serialize, Deserialize)]
pub struct Reference {
    note_id: String,
    to_note: String,
    loc: String,
    line: Option<usize>,
}

impl Reference {
    fn from_link(l: note::Ref) -> Result<Self> {
        let (loc, line) = match l.loc {
            note::RefLoc::FrontMatter => ("frontmatter".to_string(), None),
            note::RefLoc::Body(x) => ("body".to_string(), Some(x)),
        };

        if let note::RefNode::Note(note_id) = l.from
            && let note::RefNode::Note(to_note) = l.to
        {
            Ok(Self {
                note_id,
                to_note,
                loc,
                line,
            })
        } else {
            Err(MyError::ParseError("link is weird".to_string()))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    tag: String,
    note_id: String,
    loc: String,
    line: Option<usize>,
}

impl Tag {
    fn from_tag(r: note::Ref) -> Result<Self> {
        let (loc, line) = match r.loc {
            note::RefLoc::FrontMatter => ("frontmatter".to_string(), None),
            note::RefLoc::Body(x) => ("body".to_string(), Some(x)),
        };

        if let note::RefNode::Note(note_id) = r.from
            && let note::RefNode::Tag(tag) = r.to
        {
            Ok(Self {
                tag,
                note_id,
                loc,
                line,
            })
        } else {
            Err(MyError::ParseError("Tag is weird".to_string()))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Content {
    note_id: String,
    text: String,
    line_num: usize,
}

impl Content {
    fn from_content_line(c: note::ContentLine) -> Self {
        Self {
            note_id: c.note_id,
            text: c.text,
            line_num: c.line_num,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Note {
    id: String,
    note_type: String,
    alias: Option<String>,
    lines: usize,
    frontmatter_lines: usize,
    path: String,
    hash: u64,
    modified: SystemTime,
}

impl Note {
    fn from_note(n: note::Note) -> Self {
        Self {
            id: n.id,
            note_type: n.note_type.to_string(),
            alias: n.aliases,
            lines: n.lines,
            frontmatter_lines: n.frontmatter_lines,
            path: n.path.to_str().unwrap_or("").to_string(),
            modified: n.modified,
            hash: n.hash,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Parent {
    parent: String,
    child: String,
}

impl Parent {
    pub fn new(parent: String, child: String) -> Self {
        Self { parent, child }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Author {
    source: String,
    author: String,
}

impl Author {
    pub fn new(source: String, author: String) -> Self {
        Self { source, author }
    }
}

#[derive(Debug, Clone)]
pub enum DbValue {
    List(Vec<DbValue>),
    Table(Vec<(String, DbValue)>),
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
}

impl duckdb::types::FromSql for DbValue {
    fn column_result(value: duckdb::types::ValueRef<'_>) -> duckdb::types::FromSqlResult<Self> {
        DbValue::from_value(value.to_owned())
    }
}

impl DbValue {
    fn from_value(value: duckdb::types::Value) -> duckdb::types::FromSqlResult<Self> {
        Ok(match value {
            duckdb::types::Value::Text(x) | duckdb::types::Value::Enum(x) => DbValue::String(x),
            duckdb::types::Value::List(values) | duckdb::types::Value::Array(values) => {
                DbValue::List(
                    values
                        .into_iter()
                        .filter_map(|v| DbValue::from_value(v).ok())
                        .collect(),
                )
            }
            duckdb::types::Value::Struct(ordered_map) => DbValue::Table({
                ordered_map
                    .iter()
                    .filter_map(|(x, y)| {
                        Some((x.to_owned(), DbValue::from_value(y.to_owned()).ok()?))
                    })
                    .collect()
            }),
            duckdb::types::Value::HugeInt(x) => DbValue::Int(x.try_into().unwrap()),
            duckdb::types::Value::BigInt(x) => DbValue::Int(x),
            duckdb::types::Value::UTinyInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::SmallInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::Int(x) => DbValue::Int(x.into()),
            duckdb::types::Value::TinyInt(x) => DbValue::Int(x.into()),

            duckdb::types::Value::USmallInt(x) => DbValue::Int(x.into()),

            duckdb::types::Value::UInt(x) => DbValue::Int(x.into()),

            duckdb::types::Value::UBigInt(x) => DbValue::Int(x.try_into().unwrap()),

            duckdb::types::Value::Boolean(x) => DbValue::Bool(x),
            duckdb::types::Value::Float(x) => DbValue::Float(x.into()),
            duckdb::types::Value::Double(x) => DbValue::Float(x),
            // duckdb::types::Value::Timestamp(time_unit, _) => todo!(),
            // duckdb::types::Value::Blob(items) => todo!(),
            // duckdb::types::Value::Date32(_) => todo!(),
            // duckdb::types::Value::Time64(time_unit, _) => todo!(),
            x => DbValue::None,
        })
    }
}

pub struct Db {
    pub(crate) connection: duckdb::Connection,
    pub(crate) notes_path: PathBuf,
    pub(crate) db_path: PathBuf,
}

impl Db {
    pub fn init(notes_dir: &std::path::Path, db_path: &std::path::Path) -> Result<Self> {
        let db = Self {
            connection: duckdb::Connection::open(db_path)?,
            db_path: db_path.to_path_buf(),
            notes_path: notes_dir.to_path_buf(),
        };

        if db
            .connection
            .prepare("select path from metadata")
            .and_then(|mut stmt| {
                // Returns the path last used by the database (to
                // handle different notes directories)
                stmt.query_row([], |row| {
                    let path: String = row.get(0)?;
                    Ok(path)
                })
            })
            .inspect_err(|x| eprint!("Error: {}", x))
            .map_or(true, |x| x != notes_dir.to_str().unwrap_or(""))
        // If we error or have a different path, recreate tables
        {
            // TODO: Messages
            // println!("Creating DB tables: {}", db.db_path.to_string_lossy());
            db.clear()?;
        }

        Ok(db)
    }

    pub fn process_files(&self) -> Result<String> {
        let (to_remove, to_add, message) = Paths::go(self)?;
        self.update_notes(to_add, to_remove)?;
        Ok(message)
    }

    fn update_notes(&self, mut to_add: Vec<note::Note>, to_remove: Vec<String>) -> Result<()> {
        let mut notes: Vec<Note> = vec![];
        let mut tags: Vec<Tag> = vec![];
        let mut references: Vec<Reference> = vec![];
        let mut parent: Vec<Parent> = vec![];
        let mut author: Vec<Author> = vec![];
        let mut content: Vec<Content> = vec![];

        while let Some(mut n) = to_add.pop() {
            while let Some(t) = n.tags.pop() {
                tags.push(Tag::from_tag(t)?);
            }
            while let Some(r) = n.links.pop() {
                references.push(Reference::from_link(r)?);
            }
            while let Some(c) = n.contents.pop() {
                content.push(Content::from_content_line(c));
            }
            while let Some(p) = n.source_parents.pop() {
                parent.push(Parent::new(p, n.id.clone()));
            }
            while let Some(a) = n.author.pop() {
                author.push(Author::new(n.id.clone(), a));
            }
            notes.push(Note::from_note(n));
        }

        if !to_remove.is_empty() {
            let placeholders: Vec<&str> = std::iter::repeat_n("?", to_remove.len()).collect();
            self.connection.execute(
                &format!("delete from tags where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&to_remove),
            )?;

            self.connection.execute(
                &format!("delete from contents where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&to_remove),
            )?;

            self.connection.execute(
                &format!("delete from authors where source in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&to_remove),
            )?;

            self.connection.execute(
                &format!("delete from parents where child in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&to_remove),
            )?;

            self.connection.execute(
                &format!("delete from refs where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&to_remove),
            )?;

            self.connection.execute(
                &format!(
                    "delete from notes where path in ({})",
                    placeholders.join(", ")
                ),
                params_from_iter(&to_remove),
            )?;
        }

        if !notes.is_empty() {
            let mut app_notes = self.connection.appender("notes")?;
            let mut app_tags = self.connection.appender("tags")?;
            let mut app_authors = self.connection.appender("authors")?;
            let mut app_parents = self.connection.appender("parents")?;
            let mut app_references = self.connection.appender("refs")?;
            let mut app_contents = self.connection.appender("contents")?;

            app_notes.append_rows(notes.into_iter().filter_map(|x| {
                Some((
                    x.id,
                    x.alias,
                    x.note_type,
                    x.path,
                    x.hash,
                    x.modified.duration_since(SystemTime::UNIX_EPOCH).ok()?,
                    x.frontmatter_lines,
                    x.lines,
                ))
            }))?;

            app_notes.flush()?;

            app_authors.append_rows(author.into_iter().map(|x| (x.source, x.author)))?;

            app_parents.append_rows(parent.into_iter().map(|x| (x.parent, x.child)))?;

            app_references.append_rows(references.into_iter().map(|reference| {
                (
                    reference.note_id,
                    reference.to_note,
                    reference.loc,
                    reference.line,
                )
            }))?;

            app_tags.append_rows(
                tags.into_iter()
                    .map(|tag| (tag.tag, tag.note_id, tag.loc, tag.line)),
            )?;

            app_contents.append_rows(
                content
                    .into_iter()
                    .map(|content| (content.note_id, content.line_num, content.text)),
            )?;

            app_authors.flush()?;
            app_tags.flush()?;
            app_references.flush()?;
            app_parents.flush()?;
            app_contents.flush()?;
        };

        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.connection
            .execute_batch(include_str!("../sql/clear.sql"))?;
        self.connection.execute(
            "insert into metadata values (?)",
            [self.notes_path.to_str().unwrap_or("").to_string()], // empty list of parameters
        )?;
        Ok(())
    }

    pub fn get_hashes(&self) -> Result<HashMap<String, u64>> {
        let mut map = HashMap::new();
        let mut stmt = self.connection.prepare("select path, hash from notes")?;

        let mut x = stmt.query([])?;

        while let Some(y) = x.next()? {
            map.insert(y.get(0)?, y.get(1)?);
        }

        Ok(map)
    }

    pub fn get_tag_notes(&self, tag: String) -> Result<DbValue> {
        self.run_and_return_list(
            r#"
select
notes.note_id,
notes.note_title,
list(distinct { 'id': tags.tag, 'line': tags.line }) as tags,
list(distinct { 'id': author_notes.note_id, 'name': author_notes.note_title }) as authors,
list(distinct { 'id': refs.to_id, 'line': refs.line}) as links,
any_value(notes.note_type),
from notes
left join tags on notes.note_id = tags.note_id
left join authors on notes.note_id = authors.source
left join notes as author_notes on authors.author = author_notes.note_id
left join refs on notes.note_id = refs.note_id
full join parents on notes.note_id = parents.parent
where notes.note_id in (select note_id from tags where tag = ?)
group by notes.note_title,notes.note_id;
"#,
            [tag],
        )
    }

    pub fn get_backlinks(&self, id: String) -> Result<DbValue> {
        self.run_and_return_list(
            r#"
select
notes.note_id,
notes.note_title,
list(distinct { 'id': tags.tag, 'line': tags.line }) as tags,
list(distinct { 'id': author_notes.note_id, 'name': author_notes.note_title }) as authors,
list(distinct { 'id': refs.to_id, 'line': refs.line}) as links,
any_value(notes.note_type),
from notes
left join tags on notes.note_id = tags.note_id
left join authors on notes.note_id = authors.source
left join notes as author_notes on authors.author = author_notes.note_id
left join refs on notes.note_id = refs.note_id
full join parents on notes.note_id = parents.parent
where notes.note_id in (select note_id from refs where to_id = ?)
group by notes.note_title,notes.note_id;
"#,
            [id],
        )
    }

    pub fn run_and_return_list<P: Params>(&self, sql: &str, params: P) -> Result<DbValue> {
        let mut stmt = self.connection.prepare(sql)?;

        let mut res: Vec<DbValue> = vec![];
        let mut rows = stmt.query(params)?;
        let column_names = rows.as_ref().unwrap().column_names();

        while let Some(row) = rows.next()? {
            res.push(DbValue::Table(
                column_names
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(x, name)| (name, row.get(x).unwrap()))
                    .collect(),
            ));
        }

        Ok(DbValue::List(res))
    }

    pub fn run_and_return_table<P: Params>(&self, sql: &str, params: P) -> Result<DbValue> {
        let mut stmt = self.connection.prepare(sql)?;

        let mut res: Vec<(String, DbValue)> = vec![];
        let mut rows = stmt.query(params)?;
        let column_names = rows.as_ref().unwrap().column_names();

        while let Some(row) = rows.next()? {
            res.push((
                row.get(0)?,
                DbValue::Table(
                    column_names
                        .clone()
                        .into_iter()
                        .enumerate()
                        .map(|(x, name)| (name, row.get(x).unwrap()))
                        .collect(),
                ),
            ));
        }

        Ok(DbValue::Table(res))
    }

    pub fn get_tag_list(&self) -> Result<DbValue> {
        self.run_and_return_list(
            "select tag, count(distinct tags.note_id) as count from tags group by tags.tag",
            [],
        )
    }

    pub fn get_notes(&self) -> Result<DbValue> {
        let x = self.run_and_return_list(
            r#"select notes.note_id as id,
              notes.note_title as title,
              any_value(notes.modified) as modified, any_value(notes.note_type) as type,
              any_value(notes.path) as path,
              list(distinct tags.tag) as tags,
              list(distinct author_notes.note_title) as authors,
              list(distinct { 'id': refs.to_id, 'line': refs.line}) as links,
              from notes
              left join tags on notes.note_id = tags.note_id
              left join authors on notes.note_id = authors.source
              left join notes as author_notes on authors.author = author_notes.note_id
              left join refs on notes.note_id = refs.note_id
              full join parents on notes.note_id = parents.parent
              group by notes.note_title, notes.note_id
              order by modified desc
"#,
            [],
        )?;
        // TODO: Messages
        // println!("{:?}", x);
        Ok(x)
    }
}
