use crate::db;
use crate::{MyError, Result, db::Frontmatter};
use chrono::offset::Local;
use std::fs;
use std::io::Write;

fn friendly_string(s: &String) -> String {
    if s.contains([':', '{', '}', '[', ']', ',', '&', '*', '#', '?', '|', '-', '<', '>', '=', '!', '%']) {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

pub fn journal(notes_dir: &String, contents: Option<Vec<String>>) -> Result<(String, String)> {
    let current_datetime = Local::now();

    let id = format!("j_{}", current_datetime.format("%Y-%m-%d"));
    let title = format!("Journal {}", current_datetime.format("%A %d %B %Y"));

    let path = format!("{}/journals/{}.md", notes_dir, id);

    let new = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(&title),
        note_type: "journal".to_string(),
        author: None,
        source_parents: None,
        references: None,
        tags: db::List(vec![]),
    };

    write_file(&path, new, contents)?;

    Ok((path, id))
}

pub fn author(
    id: &String,
    title: &String,
    notes_dir: &String,
    contents: Option<Vec<String>>,
) -> Result<(String, String)> {
    let id = format!("a_{}", &id);
    let path = format!("{}/authors/{}.md", notes_dir, id);

    let child = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(title),
        note_type: "author".to_string(),
        author: None,
        source_parents: None,
        references: None,
        tags: db::List(vec![]),
    };

    write_file(&path, child, contents)?;

    Ok((path, id))
}

pub fn note(
    parent: Frontmatter,
    id: &String,
    title: &String,
    notes_dir: &String,
    contents: Option<Vec<String>>,
) -> Result<(String, String)> {
    let path = format!("{}/notes/{}.md", notes_dir, id);

    let note = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(title),
        note_type: "note".to_string(),
        author: None,
        source_parents: None,
        references: parent.references,
        tags: parent.tags,
    };

    write_file(&path, note, contents)?;

    Ok((path, id.to_string()))
}
pub fn source(
    parent: Frontmatter,
    child_id: &String,
    child_title: &String,
    notes_dir: &String,
    contents: Option<Vec<String>>,
) -> Result<(String, String)> {
    let (id, authors, source_parents) = match parent.note_type.as_str() {
        "author" => {
            let id = format!("s_{}_{}", parent.id.trim_prefix("a_"), &child_id);
            let authors = db::List(vec![parent.id]);
            let source_parents = db::List(vec![]);
            Ok((id, authors, source_parents))
        }
        "source" => {
            let id = format!("{}_{}", parent.id, &child_id);
            let authors = parent.author.unwrap_or(db::List(vec![]));
            let mut source_parents = parent.source_parents.unwrap_or(db::List(vec![]));

            source_parents.push(parent.id);

            Ok((id, authors, source_parents))
        }
        _ => Err(MyError::OtherError),
    }?;

    let path = format!("{}/sources/{}.md", notes_dir, id);

    let child = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(child_title),
        note_type: "source".to_string(),
        author: Some(authors),
        source_parents: Some(source_parents),
        references: parent.references,
        tags: parent.tags,
    };

    write_file(&path, child, contents)?;

    Ok((path, id))
}

fn write_file(path: &String, f: Frontmatter, contents: Option<Vec<String>>) -> Result<()> {
    let mut file = fs::File::create_new(path)?;
    let mut c = f.to_yaml();

    if let Some(mut lines) = contents {
        c.append(&mut lines)
    }

    file.write_all(c.join("\n").as_bytes())?;

    Ok(())
}
