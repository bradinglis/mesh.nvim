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

use serde::{Deserialize, Serialize};

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
        let x = match self.len() {
            0 => "[]".to_string(),
            _ => self.join(" "),
        };
        f.write_fmt(format_args!("{}", x))
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


#[derive(Serialize, Deserialize)]
pub struct Reference {
    note_id: String,
    kind: String,
    to_anchor: String,
    line: Option<usize>,
    start_col: Option<usize>,
    end_col: Option<usize>,
}

impl Reference {
    fn from_link(l: note::Ref) -> Result<Self> {
        let (kind, line, start_col, end_col) = match l.loc {
            note::RefLoc::FrontMatter(x, y, z) => (
                "frontmatter_reference".to_string(),
                Some(x),
                Some(y),
                Some(z),
            ),
            note::RefLoc::Body(x, y, z) => ("intext_link".to_string(), Some(x), Some(y), Some(z)),
        };

        if let note::RefNode::NoteAnchor(note_id) = l.from
            && let note::RefNode::NoteAnchor(to_anchor) = l.to
        {
            Ok(Self {
                kind,
                note_id,
                to_anchor,
                line,
                start_col,
                end_col,
            })
        } else {
            Err(MyError::ParseError("link is weird".to_string()))
        }
    }

    fn from_author(l: note::Ref) -> Result<Self> {
        let (kind, line, start_col, end_col) = match l.loc {
            note::RefLoc::FrontMatter(x, y, z) => ("author".to_string(), Some(x), Some(y), Some(z)),
            note::RefLoc::Body(x, y, z) => ("author".to_string(), Some(x), Some(y), Some(z)),
        };

        if let note::RefNode::NoteAnchor(note_id) = l.from
            && let note::RefNode::NoteAnchor(to_anchor) = l.to
        {
            Ok(Self {
                kind,
                note_id,
                to_anchor,
                line,
                start_col,
                end_col,
            })
        } else {
            Err(MyError::ParseError("author is weird".to_string()))
        }
    }

    fn from_parent(l: note::Ref) -> Result<Self> {
        let (kind, line, start_col, end_col) = match l.loc {
            note::RefLoc::FrontMatter(x, y, z) => ("parent".to_string(), Some(x), Some(y), Some(z)),
            note::RefLoc::Body(x, y, z) => ("parent".to_string(), Some(x), Some(y), Some(z)),
        };

        if let note::RefNode::NoteAnchor(note_id) = l.from
            && let note::RefNode::NoteAnchor(to_anchor) = l.to
        {
            Ok(Self {
                kind,
                note_id,
                to_anchor,
                line,
                start_col,
                end_col,
            })
        } else {
            Err(MyError::ParseError("author is weird".to_string()))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    tag: String,
    note_id: String,
    loc: String,
    line: Option<usize>,
    start_col: Option<usize>,
    end_col: Option<usize>,
}

impl Tag {
    fn from_tag(r: note::Ref) -> Result<Self> {
        let (loc, line, start_col, end_col) = match r.loc {
            note::RefLoc::FrontMatter(x, y, z) => {
                ("frontmatter".to_string(), Some(x), Some(y), Some(z))
            }
            note::RefLoc::Body(x, y, z) => ("body".to_string(), Some(x), Some(y), Some(z)),
        };

        if let note::RefNode::NoteAnchor(note_id) = r.from
            && let note::RefNode::Tag(tag) = r.to
        {
            Ok(Self {
                tag,
                note_id,
                loc,
                line,
                start_col,
                end_col,
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
    fn from_note(n: note::Note) -> Result<Self> {
        Ok(Self {
            id: n.id,
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

#[derive(Serialize, Deserialize)]
pub struct Anchor {
    note_id: String,
    anchor_id: String,
    title: String,
    line: Option<usize>,
    start_col: Option<usize>,
    end_col: Option<usize>,
}

impl Anchor {
    pub fn from_heading(from: note::Anchor) -> Self {
        let (_, line, start_col, end_col) = match from.loc {
            note::RefLoc::FrontMatter(x, y, z) => {
                ("frontmatter".to_string(), Some(x), Some(y), Some(z))
            }
            note::RefLoc::Body(x, y, z) => ("body".to_string(), Some(x), Some(y), Some(z)),
        };
        Anchor {
            note_id: from.note_id,
            anchor_id: from.heading_link,
            title: from.title,
            line,
            start_col,
            end_col,
        }
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
            _x => DbValue::None,
        })
    }
}

pub struct Db {
    pub(crate) connection: duckdb::Connection,
    pub(crate) notes_path: PathBuf,
    // pub(crate) db_path: PathBuf,
}

impl Db {
    pub fn init(notes_dir: &std::path::Path, db_path: &std::path::Path) -> Result<Self> {
        let db = Self {
            connection: duckdb::Connection::open(db_path)?,
            // db_path: db_path.to_path_buf(),
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
            .map_or(true, |x| x != notes_dir.to_str().unwrap_or(""))
        {
            db.clear()?;
        }

        Ok(db)
    }

    pub fn process_files(&self) -> Result<String> {
        let start = Instant::now();

        let (to_remove, to_add, mut message) = Paths::go(self)?;
        if !(to_remove.is_empty() && to_add.is_empty()) {
            self.update_notes(to_add, to_remove)?;
        }

        message.push(format!("Time Taken: {}s", start.elapsed().as_secs_f64()));
        Ok(message.join("\n"))
    }

    fn update_notes(&self, mut to_add: Vec<note::Note>, to_remove: Vec<String>) -> Result<()> {
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
                &format!("delete from refs where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&to_remove),
            )?;

            self.connection.execute(
                &format!("delete from anchors where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
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

        if !to_add.is_empty() {
            let mut notes: Vec<Note> = vec![];
            let mut tags: Vec<Tag> = vec![];
            let mut references: Vec<Reference> = vec![];
            let mut headings: Vec<Anchor> = vec![];
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
                    references.push(Reference::from_parent(p)?);
                }
                while let Some(a) = n.author.pop() {
                    references.push(Reference::from_author(a)?);
                }
                while let Some(h) = n.headings.pop() {
                    headings.push(Anchor::from_heading(h));
                }
                if let Ok(note) = Note::from_note(n) {
                    notes.push(note);
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

            app_notes.flush()?;

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
                    reference.kind,
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
                    tag.loc,
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

            app_tags.flush()?;
            app_references.flush()?;
            app_contents.flush()?;
            app_headings.flush()?;
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

    pub fn get_backlinks_2(&self, id: String) -> Result<DbValue> {
        self.run_and_return_list(
            &format!(
                "select full_notes.*,
                from ({}) as full_notes
                full join (select *, 'from_ref' as type from refs where refs.to_id = $1) as from_refs on from_refs.note_id = full_notes.id
                full join (select *, 'to_ref' as type from refs where refs.note_id = $1) as to_refs on to_refs.to_id = full_notes.id
                full join (select *, 'child' as type from parents where parents.parent = $1) as source_children on source_children.child = full_notes.id
                full join (select *, 'parent' as type from parents where parents.child = $1) as source_parents on source_parents.parent = full_notes.id
                full join (select *, 'author' as type from authors where authors.source = $1) as source_authors on source_authors.author = full_notes.id
                full join (select *, 'source' as type from authors where authors.author = $1) as author_works on author_works.source = full_notes.id
                where link_type not null
                order by modified desc",
                include_str!("../sql/notes_details.sql")
            ),
            [id],
        )
    }

    pub fn get_frontmatter(&self, id: &String) -> Result<Frontmatter> {
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
            r#"select tag,
            count(distinct tags.note_id) as count,
            list(distinct {'id': tags.note_id, 'line': tags.line }) as notes
            from tags
            group by tags.tag
            order by count desc"#,
            [],
        )
    }

    // pub fn get_path(&self, id: String) -> Result<DbValue> {
    //     self.run_and_return_one(r#"select notes.path from notes where note_id = ?"#, [id])
    // }

    pub fn get_loc(
        &self,
        id: String,
    ) -> Result<(Option<DbValue>, Option<DbValue>, Option<DbValue>)> {
        let mut x = self.run_and_return_one(r#"select notes.path, anchors.line, anchors.start_col from anchors natural join notes where anchors.anchor_id = ?"#, [id])?;
        let c = x.pop();
        let b = x.pop();
        let a = x.pop();
        Ok((a, b, c))
    }

    pub fn get_notes(&self) -> Result<DbValue> {
        let x = self.run_and_return_list(include_str!("../sql/notes_details.sql"), [])?;
        // TODO: Messages
        // println!("{:?}", x);
        Ok(x)
    }

    pub fn get_note_anchors(&self, id: String) -> Result<DbValue> {
        let x = self.run_and_return_list(
            r#"
        select 
            anchors.anchor_id as id,
            anchors.title as title,
        from anchors
        where anchors.note_id = ?
        "#,
            [id],
        )?;
        Ok(x)
    }
    pub fn get_thin_notes(&self) -> Result<DbValue> {
        let x = self.run_and_return_list(
            r#"
        select 
            notes.note_id as id,
            notes.note_title as title,
            notes.modified as modified,
            notes.note_type as type,
            notes.path as path,
        from notes
        order by modified desc
        "#,
            [],
        )?;
        Ok(x)
    }
}
