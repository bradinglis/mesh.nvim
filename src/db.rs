use std::{
    collections::HashMap,
    fmt::Display,
    ops::{Deref, DerefMut},
    path::PathBuf,
    time::{Instant, SystemTime},
    vec,
};

use duckdb::types::FromSql;
use duckdb::{Params, params_from_iter};

use crate::{MyError, Result, file::Paths, note};

pub struct Frontmatter {
    pub(crate) id: String,
    pub(crate) aliases: String,
    pub(crate) note_type: String,
    pub(crate) author: Option<List>,
    pub(crate) source_parents: Option<List>,
    pub(crate) references: Option<List>,
    pub(crate) tags: List,
}

pub struct List(pub Vec<String>);

impl Frontmatter {
    pub fn to_yaml(&self) -> Vec<String> {
        let mut res: Vec<String> = Vec::new();
        res.push("---".to_string());
        res.push(format!("id: {}", self.id));
        res.push(format!("aliases: {}", self.aliases));
        res.push(format!("type: {}", self.note_type));
        if let Some(author) = &self.author {
            res.push(format!("author: {}", author));
        }
        if let Some(source_parents) = &self.source_parents {
            res.push(format!("source-parents: {}", source_parents));
        }
        if let Some(author) = &self.references {
            res.push(format!("references: {}", author));
        }
        res.push(format!("tags: {}", self.tags));
        res.push("---".to_string());
        res.push(format!("# {}", self.aliases));
        res.push("".to_string());
        res.push("".to_string());

        res
    }
}

impl Display for List {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            f.write_str("[]")
        } else {
            f.write_str(&self.join(" "))
        }
    }
}

impl FromSql for List {
    fn column_result(value: duckdb::types::ValueRef<'_>) -> duckdb::types::FromSqlResult<Self> {
        match value.to_owned() {
            duckdb::types::Value::List(x) => {
                let y: Vec<String> = x
                    .into_iter()
                    .filter_map(|z| {
                        if let duckdb::types::Value::Text(a) = z {
                            Some(a)
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(List(y))
            }
            _ => Err(duckdb::types::FromSqlError::InvalidType),
        }
    }
}

impl Deref for List {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for List {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The relationship a reference expresses. Kept as an enum so new link kinds
/// cannot be introduced with a typo in a string literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefKind {
    InTextLink,
    FrontmatterReference,
    Author,
    Parent,
}

impl RefKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InTextLink => "intext_link",
            Self::FrontmatterReference => "frontmatter_reference",
            Self::Author => "author",
            Self::Parent => "parent",
        }
    }
}

#[derive(Debug)]
pub struct Reference {
    note_id: String,
    kind: RefKind,
    to_anchor: String,
    line: Option<usize>,
    start_col: Option<usize>,
    end_col: Option<usize>,
}

impl Reference {
    fn from_link(reference: note::Ref) -> Result<Self> {
        let kind = if reference.loc.is_frontmatter() {
            RefKind::FrontmatterReference
        } else {
            RefKind::InTextLink
        };
        Self::from_ref(reference, kind)
    }

    fn from_author(reference: note::Ref) -> Result<Self> {
        Self::from_ref(reference, RefKind::Author)
    }

    fn from_parent(reference: note::Ref) -> Result<Self> {
        Self::from_ref(reference, RefKind::Parent)
    }

    fn from_ref(reference: note::Ref, kind: RefKind) -> Result<Self> {
        let (line, start_col, end_col) = reference.loc.get_loc();

        match (reference.from, reference.to) {
            (Some(note::RefNode::NoteAnchor(note_id)), note::RefNode::NoteAnchor(to_anchor)) => {
                Ok(Self {
                    kind,
                    note_id,
                    to_anchor,
                    line: Some(line),
                    start_col: Some(start_col),
                    end_col: Some(end_col),
                })
            }
            (from, to) => Err(MyError::ParseError(format!(
                "Invalid {kind:?} reference from {from:?} to {to:?}"
            ))),
        }
    }
}

/// Where a tag or anchor was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Location {
    Frontmatter,
    Body,
}

impl Location {
    const fn from_loc(loc: note::RefLoc) -> Self {
        if loc.is_frontmatter() {
            Self::Frontmatter
        } else {
            Self::Body
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Frontmatter => "frontmatter",
            Self::Body => "body",
        }
    }
}

#[derive(Debug)]
pub struct Tag {
    tag: String,
    note_id: String,
    loc: Location,
    line: Option<usize>,
    start_col: Option<usize>,
    end_col: Option<usize>,
}

impl Tag {
    fn from_tag(reference: note::Ref) -> Result<Self> {
        let loc = Location::from_loc(reference.loc);
        let (line, start_col, end_col) = reference.loc.get_loc();

        match (reference.from, reference.to) {
            (Some(note::RefNode::NoteAnchor(note_id)), note::RefNode::Tag(tag)) => Ok(Self {
                tag,
                note_id,
                loc,
                line: Some(line),
                start_col: Some(start_col),
                end_col: Some(end_col),
            }),
            (from, to) => Err(MyError::ParseError(format!(
                "Invalid tag reference from {from:?} to {to:?}"
            ))),
        }
    }
}

#[derive(Debug)]
pub struct Content {
    note_id: String,
    text: String,
    line_num: usize,
}

impl Content {
    fn from_content_line(c: note::ContentLine) -> Result<Self> {
        Ok(Self {
            note_id: c.note_id.ok_or(MyError::OtherError)?,
            text: c.text,
            line_num: c.line_num,
        })
    }
}

#[derive(Debug)]
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
    fn from_note(n: note::Note) -> Result<Self> {
        Ok(Self {
            id: n.id.ok_or(MyError::OtherError)?,
            note_type: n.note_type.to_string(),
            alias: n.title,
            lines: n.lines,
            frontmatter_lines: n.frontmatter_lines,
            path: n
                .path
                .map(|x| x.to_str().unwrap_or("").to_string())
                .unwrap_or("".to_string()),
            modified: n
                .modified
                .ok_or(MyError::FileError("No modified time".to_string()))?,
            hash: n
                .hash
                .ok_or(MyError::FileError("No file hash".to_string()))?,
        })
    }
}

#[derive(Debug)]
pub struct Anchor {
    note_id: String,
    anchor_id: String,
    title: String,
    line: Option<usize>,
    start_col: Option<usize>,
    end_col: Option<usize>,
}

impl Anchor {
    pub fn from_heading(from: note::Anchor) -> Result<Self> {
        let (line, start_col, end_col) = from.loc.get_loc();
        Ok(Anchor {
            note_id: from.note_id.ok_or(MyError::OtherError)?,
            anchor_id: from.heading_link,
            title: from.title,
            line: Some(line),
            start_col: Some(start_col),
            end_col: Some(end_col),
        })
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
            duckdb::types::Value::HugeInt(x) => {
                i64::try_from(x).map(DbValue::Int).unwrap_or(DbValue::None)
            }
            duckdb::types::Value::BigInt(x) => DbValue::Int(x),
            duckdb::types::Value::UTinyInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::SmallInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::Int(x) => DbValue::Int(x.into()),
            duckdb::types::Value::TinyInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::USmallInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::UInt(x) => DbValue::Int(x.into()),
            duckdb::types::Value::UBigInt(x) => {
                i64::try_from(x).map(DbValue::Int).unwrap_or(DbValue::None)
            }

            duckdb::types::Value::Boolean(x) => DbValue::Bool(x),
            duckdb::types::Value::Float(x) => DbValue::Float(x.into()),
            duckdb::types::Value::Double(x) => DbValue::Float(x),
            _x => DbValue::None,
        })
    }
}

pub struct Db {
    pub(crate) connection: duckdb::Connection,
    pub(crate) notes_path: PathBuf,
    // pub(crate) errors: Vec<MyError>,
}

impl Db {
    pub fn init(notes_dir: &std::path::Path, db_path: &std::path::Path) -> Result<Self> {
        let db = Self {
            connection: duckdb::Connection::open(db_path)?,
            notes_path: notes_dir.to_path_buf(),
            // errors: vec![],
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
            .ok()
            .is_none_or(|path| path != notes_dir.to_str().unwrap_or(""))
        {
            db.clear()?;
        }

        Ok(db)
    }

    // Want this to return error as tuple/struct
    pub fn process_files(&self) -> Result<String> {
        let start = Instant::now();

        let (to_remove, to_add, mut message) = Paths::go(self)?;
        if !(to_remove.is_empty() && to_add.is_empty()) {
            match self.update_notes(to_add, to_remove) {
                Ok(mut x) => message.append(&mut x),
                Err(e) => message.push(format!("Error: {}", e)),
            }
        }

        message.push(format!("Time Taken: {}s", start.elapsed().as_secs_f64()));
        Ok(message.join("\n"))
    }

    fn update_notes(&self, to_add: Vec<note::Note>, to_remove: Vec<String>) -> Result<Vec<String>> {
        let mut messages = Vec::new();

        if !to_remove.is_empty() {
            let placeholders = vec!["?"; to_remove.len()].join(", ");
            let note_ids = format!("select note_id from notes where path in ({placeholders})");

            for table in ["tags", "contents", "refs", "anchors"] {
                self.connection.execute(
                    &format!("delete from {table} where note_id in ({note_ids})"),
                    params_from_iter(&to_remove),
                )?;
            }

            self.connection.execute(
                &format!("delete from notes where path in ({placeholders})"),
                params_from_iter(&to_remove),
            )?;
        }

        if to_add.is_empty() {
            return Ok(messages);
        }

        let mut notes: Vec<Note> = Vec::new();
        let mut tags: Vec<Tag> = Vec::new();
        let mut references: Vec<Reference> = Vec::new();
        let mut headings: Vec<Anchor> = Vec::new();
        let mut content: Vec<Content> = Vec::new();

        for mut note in to_add {
            for tag in note.tags.drain(..) {
                match Tag::from_tag(tag) {
                    Ok(tag) => tags.push(tag),
                    Err(error) => messages.push(format!("Error: {error}")),
                }
            }
            for link in note.links.drain(..) {
                match Reference::from_link(link) {
                    Ok(reference) => references.push(reference),
                    Err(error) => messages.push(format!("Error: {error}")),
                }
            }
            for line in note.contents.drain(..) {
                if let Ok(line) = Content::from_content_line(line) {
                    content.push(line);
                }
            }
            for parent in note.source_parents.drain(..) {
                match Reference::from_parent(parent) {
                    Ok(reference) => references.push(reference),
                    Err(error) => messages.push(format!("Error: {error}")),
                }
            }
            for author in note.author.drain(..) {
                match Reference::from_author(author) {
                    Ok(reference) => references.push(reference),
                    Err(error) => messages.push(format!("Error: {error}")),
                }
            }
            for heading in note.headings.drain(..) {
                match Anchor::from_heading(heading) {
                    Ok(anchor) => headings.push(anchor),
                    Err(error) => messages.push(format!("Error: {error}")),
                }
            }
            match Note::from_note(note) {
                Ok(note) => notes.push(note),
                Err(error) => messages.push(format!("Error: {error}")),
            }
        }

        let mut app_notes = self.connection.appender("notes")?;
        let mut app_tags = self.connection.appender("tags")?;
        let mut app_references = self.connection.appender("refs")?;
        let mut app_contents = self.connection.appender("contents")?;
        let mut app_headings = self.connection.appender("anchors")?;

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

        app_headings.append_rows(headings.into_iter().map(|x| {
            (
                x.note_id,
                x.anchor_id,
                x.title,
                x.line,
                x.start_col,
                x.end_col,
            )
        }))?;

        app_references.append_rows(references.into_iter().map(|reference| {
            (
                reference.note_id,
                reference.kind.as_str(),
                reference.to_anchor,
                reference.line,
                reference.start_col,
                reference.end_col,
            )
        }))?;

        app_tags.append_rows(tags.into_iter().map(|tag| {
            (
                tag.tag,
                tag.note_id,
                tag.loc.as_str(),
                tag.line,
                tag.start_col,
                tag.end_col,
            )
        }))?;

        app_contents.append_rows(
            content
                .into_iter()
                .map(|content| (content.note_id, content.line_num, content.text)),
        )?;

        app_notes.flush()?;
        app_tags.flush()?;
        app_references.flush()?;
        app_contents.flush()?;
        app_headings.flush()?;

        Ok(messages)
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
            &format!(
                "select full_notes.*, tags.line, tags.start_col, tags.end_col from (select * from tags where tags.tag = ?) as tags
                inner join ({}) as full_notes on tags.note_id = full_notes.id
                order by modified desc
                ",
                include_str!("../sql/notes_details.sql")
            ),
            [tag],
        )
    }

    pub fn get_type_notes(&self, note_type: String) -> Result<DbValue> {
        self.run_and_return_list(
            &format!(
                "select * from ({}) as full_notes where type = ? order by modified desc
                ",
                include_str!("../sql/notes_details.sql")
            ),
            [note_type],
        )
    }

    pub fn get_backlinks(&self, id: String) -> Result<DbValue> {
        self.run_and_return_list(
            &format!(
                "select full_notes.*, refs.direction, refs.kind, refs.line, refs.start_col, refs.end_col
                from
                (
                select kind, line, start_col, end_col, note_id as ref, 'back' as direction from refs where refs.to_anchor = $1
                union all
                select kind, null as line, null as start_col, null as end_col, to_anchor as ref, 'forward' as direction from refs where refs.note_id = $1
                ) as refs
                inner join ({}) as full_notes on refs.ref = full_notes.id
                order by modified desc",
                include_str!("../sql/notes_details.sql")
            ),
            [id],
        )
    }

    pub fn get_frontmatter(&self, id: &str) -> Result<Frontmatter> {
        let mut stmt = self.connection.prepare(
                r#"select
                    notes.note_id as id,
                    any_value(notes.note_title) as title,
                    any_value(notes.note_type) as note_type,
                    list(distinct authors.to_anchor) as authors,
                    list(distinct parents.to_anchor) as source_parents,
                    list(distinct f_refs.to_anchor) as references,
                    list(distinct tags.tag) as tags
                from notes
                    natural left join tags 
                    left join (select * from refs where kind = 'author') as authors on authors.note_id = notes.note_id
                    left join (select * from refs where kind = 'parent') as parents on parents.note_id = notes.note_id
                    left join (select * from refs where kind = 'frontmatter_reference') as f_refs on f_refs.note_id = notes.note_id
                where id = ?
                group by notes.note_id"#,
        )?;

        Ok(stmt.query_one([id], |row| {
            Ok(Frontmatter {
                id: row.get(0)?,
                aliases: row.get(1)?,
                note_type: row.get(2)?,
                author: row.get(3)?,
                source_parents: row.get(4)?,
                references: row.get(5)?,
                tags: row.get(6)?,
            })
        })?)
    }

    pub fn run_and_return_list<P: Params>(&self, sql: &str, params: P) -> Result<DbValue> {
        let mut stmt = self.connection.prepare(sql)?;

        let mut rows = stmt.query(params)?;
        let column_names = rows.as_ref().unwrap().column_names();
        let mut res: Vec<DbValue> = Vec::new();

        while let Some(row) = rows.next()? {
            res.push(DbValue::Table(
                column_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| (name.clone(), row.get(index).unwrap_or(DbValue::None)))
                    .collect(),
            ));
        }

        Ok(DbValue::List(res))
    }

    pub fn run_and_return_one<P: Params>(&self, sql: &str, params: P) -> Result<Vec<DbValue>> {
        let mut stmt = self.connection.prepare(sql)?;
        let x: Vec<DbValue> = stmt.query_one(params, |row| {
            let mut r = vec![];
            let mut i = 0;
            while let Ok(col) = row.get(i) {
                r.push(col);
                i += 1;
            }
            Ok(r)
        })?;

        Ok(x)
    }

    pub fn run_and_return_table<P: Params>(&self, sql: &str, params: P) -> Result<DbValue> {
        let mut stmt = self.connection.prepare(sql)?;

        let mut rows = stmt.query(params)?;
        let column_names = rows.as_ref().unwrap().column_names();
        let mut res: Vec<(String, DbValue)> = Vec::new();

        while let Some(row) = rows.next()? {
            res.push((
                row.get(0)?,
                DbValue::Table(
                    column_names
                        .iter()
                        .enumerate()
                        .map(|(index, name)| {
                            (name.clone(), row.get(index).unwrap_or(DbValue::None))
                        })
                        .collect(),
                ),
            ));
        }

        Ok(DbValue::Table(res))
    }

    pub fn get_tag_list(&self) -> Result<DbValue> {
        self.run_and_return_list(
            r#"select tag,
            count(distinct tags.note_id) as count,
            list(distinct {'id': tags.note_id, 'line': tags.line }) as notes
            from tags
            group by tags.tag
            order by count desc"#,
            [],
        )
    }

    pub fn get_loc(
        &self,
        id: String,
    ) -> Result<(Option<DbValue>, Option<DbValue>, Option<DbValue>)> {
        let values = self.run_and_return_one(r#"select notes.path, anchors.line, anchors.start_col from anchors natural join notes where anchors.anchor_id = ?"#, [id])?;
        let mut values = values.into_iter();
        Ok((values.next(), values.next(), values.next()))
    }

    pub fn get_notes(&self) -> Result<DbValue> {
        let x = self.run_and_return_list(include_str!("../sql/notes_details.sql"), [])?;
        Ok(x)
    }

    pub fn get_lines(&self) -> Result<DbValue> {
        let x = self.run_and_return_list(&format!("select contents.line, contents.text, note.* from contents join ({}) as note on contents.note_id == note.id;", include_str!("../sql/notes_details.sql")), [])?;
        Ok(x)
    }

    pub fn get_note_anchors(&self, id: String) -> Result<DbValue> {
        self.run_and_return_list(
            r#"
        select
            anchors.anchor_id as id,
            anchors.title as title
        from anchors
        where anchors.note_id = ?
        "#,
            [id],
        )
    }

    pub fn get_thin_notes(&self) -> Result<DbValue> {
        self.run_and_return_list(
            r#"
        select
            notes.note_id as id,
            notes.note_title as title,
            notes.modified as modified,
            notes.note_type as type,
            notes.path as path
        from notes
        order by modified desc
        "#,
            [],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_paths() -> (std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "mesh-db-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let notes = base.join("notes");
        let dbroot = base.join("db");
        fs::create_dir_all(&notes).unwrap();
        fs::create_dir_all(&dbroot).unwrap();
        fs::write(
            notes.join("a.md"),
            "---\nid: A\naliases: A\ntype: note\ntags: []\n---\nhi\n",
        )
        .unwrap();
        (notes, dbroot.join("test.duckdb"))
    }

    #[test]
    fn init_and_process_files() {
        let (notes, db_path) = temp_paths();
        let db = Db::init(&notes, &db_path).unwrap();
        let msg = db.process_files().unwrap();
        assert!(msg.contains("New:") || msg.contains("Updated Index"));
        let msg2 = db.process_files().unwrap();
        assert!(msg2.contains("Time Taken"));
    }
}
