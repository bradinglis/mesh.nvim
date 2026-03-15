use std::{collections::HashMap, path::PathBuf, time::SystemTime};

use duckdb::{AppenderParams, Connection, ToSql, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::{MyError, Result, note};

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
            Err(MyError::OtherError)
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
            Err(MyError::OtherError)
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

#[derive(Serialize, Deserialize)]
pub struct Db {
    notes: Vec<Note>,
    tags: Vec<Tag>,
    references: Vec<Reference>,
    parent: Vec<Parent>,
    author: Vec<Author>,
    content: Vec<Content>,
    removing: Vec<String>,
}

impl Db {
    pub fn init(removing: Vec<String>) -> Self {
        Self {
            notes: vec![],
            parent: vec![],
            author: vec![],
            content: vec![],
            tags: vec![],
            references: vec![],
            removing,
        }
    }

    pub fn from_notes(mut test: Vec<note::Note>, removing: Vec<String>) -> Result<Self> {
        let mut db = Self::init(removing);
        while let Some(mut n) = test.pop() {
            while let Some(t) = n.tags.pop() {
                db.tags.push(Tag::from_tag(t)?);
            }
            while let Some(r) = n.links.pop() {
                db.references.push(Reference::from_link(r)?);
            }
            while let Some(c) = n.contents.pop() {
                db.content.push(Content::from_content_line(c));
            }
            while let Some(parent) = n.source_parents.pop() {
                db.parent.push(Parent::new(parent, n.id.clone()));
            }
            while let Some(author) = n.author.pop() {
                db.author.push(Author::new(n.id.clone(), author));
            }
            db.notes.push(Note::from_note(n));
        }
        Ok(db)
    }

    pub fn create_db(&self, conn: &Connection) -> Result<()> {
        conn.execute("DROP TABLE if exists tags", [])?;
        conn.execute("DROP TABLE if exists parents", [])?;
        conn.execute("DROP TABLE if exists refs", [])?;
        conn.execute("DROP TABLE if exists authors", [])?;
        conn.execute("DROP TABLE if exists contents", [])?;
        conn.execute("DROP TABLE if exists notes CASCADE", [])?;

        conn.execute(
            "CREATE OR REPLACE TABLE notes (
            note_id TEXT PRIMARY KEY NOT NULL,
            note_title TEXT NULL,
            note_type TEXT NOT NULL,
            path TEXT NOT NULL,
            hash UBIGINT NOT NULL,
            modified TIMESTAMP NOT NULL,
            frontmatterlines INTEGER NOT NULL,
            lines INTEGER NOT NULL,
        )",
            [], // empty list of parameters
        )?;

        conn.execute(
            "CREATE OR REPLACE TABLE tags (
            tag TEXT NOT NULL,
            note_id TEXT NOT NULL,
            loc TEXT NOT NULL,
            line UINTEGER NULL,
            CONSTRAINT fk_Note FOREIGN KEY (note_id) REFERENCES notes(note_id)
        )",
            [], // empty list of parameters
        )?;

        conn.execute(
            "CREATE OR REPLACE TABLE authors (
            source TEXT NOT NULL,
            author TEXT NOT NULL,
            CONSTRAINT fk_Source FOREIGN KEY (source) REFERENCES notes(note_id),
            CONSTRAINT fk_Author FOREIGN KEY (author) REFERENCES notes(note_id),
            PRIMARY KEY (source, author)
        )",
            [], // empty list of parameters
        )?;

        conn.execute(
            "CREATE OR REPLACE TABLE parents (
            parent TEXT NOT NULL,
            child TEXT NOT NULL,
            CONSTRAINT fk_Child FOREIGN KEY (child) REFERENCES notes(note_id),
            PRIMARY KEY (parent, child)
        )",
            [], // empty list of parameters
        )?;
        // CONSTRAINT fk_Parent FOREIGN KEY (parent) REFERENCES notes(note_id),

        conn.execute(
            "CREATE OR REPLACE TABLE refs (
            note_id TEXT NOT NULL,
            to_id TEXT NOT NULL,
            loc TEXT NOT NULL,
            line UINTEGER NULL,
            CONSTRAINT fk_FromNote FOREIGN KEY (note_id) REFERENCES notes(note_id)
        )",
            [], // empty list of parameters
        )?;
        // CONSTRAINT fk_ToNote FOREIGN KEY (to_id) REFERENCES notes(id)

        conn.execute(
            "CREATE OR REPLACE TABLE contents (
            note_id TEXT NOT NULL,
            line UINTEGER NOT NULL,
            text TEXT NOT NULL,
            CONSTRAINT fk_Note FOREIGN KEY (note_id) REFERENCES notes(note_id),
            PRIMARY KEY (note_id, line)
        )",
            [], // empty list of parameters
        )?;

        Ok(())
    }

    pub fn commit_db(mut self, conn: Connection) -> Result<()> {

        let placeholders: Vec<&str> = std::iter::repeat_n("?", self.removing.len()).collect();

        conn.execute(
                &format!("delete from tags where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&self.removing),
            )?;

        conn.execute(
                &format!("delete from contents where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&self.removing),
            )?;

        conn.execute(
                &format!("delete from authors where source in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&self.removing),
            )?;

        conn.execute(
                &format!("delete from parents where child in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&self.removing),
            )?;

        conn.execute(
                &format!("delete from refs where note_id in (select note_id from notes where path in ({}))", placeholders.join(", ")),
                params_from_iter(&self.removing),
            )?;

        conn.execute(
                &format!("delete from notes where path in ({})", placeholders.join(", ")),
                params_from_iter(&self.removing),
            )?;

        // let mut rows = stmt.query(params_from_iter(self.removing))?;

        let mut app_notes = conn.appender("notes")?;
        let mut app_tags = conn.appender("tags")?;
        let mut app_authors = conn.appender("authors")?;
        let mut app_parents = conn.appender("parents")?;
        let mut app_references = conn.appender("refs")?;
        let mut app_contents = conn.appender("contents")?;

        app_notes.append_rows(self.notes.into_iter().filter_map(|x| {
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

        app_authors.append_rows(self.author.into_iter().map(|x| (x.source, x.author)))?;

        app_parents.append_rows(self.parent.into_iter().map(|x| (x.parent, x.child)))?;

        app_references.append_rows(self.references.into_iter().map(|reference| {
            (
                reference.note_id,
                reference.to_note,
                reference.loc,
                reference.line,
            )
        }))?;

        app_tags.append_rows(
            self.tags
                .into_iter()
                .map(|tag| (tag.tag, tag.note_id, tag.loc, tag.line)),
        )?;

        app_contents.append_rows(
            self.content
                .into_iter()
                .map(|content| (content.note_id, content.line_num, content.text)),
        )?;

        app_authors.flush()?;
        app_tags.flush()?;
        app_references.flush()?;
        app_parents.flush()?;
        app_contents.flush()?;

        Ok(())
    }
}

// fn tags(test: Vec<note::Note>) -> HashMap<String, Vec<note::Ref>> {
//     let grouped: HashMap<String, Vec<note::Ref>> =
//         test.into_iter().fold(HashMap::new(), |mut acc, val| {
//             val.tags.into_iter().for_each(|tag| {
//                 if let Ok(x) = tag.get_tag() {
//                     acc.entry(x).or_default().push(tag);
//                 }
//             });
//             acc
//         });
//     grouped
// }
