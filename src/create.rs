use crate::note::NoteType;
use crate::{MyError, Result, db, db::Frontmatter};
use chrono::offset::Local;
use std::fs;
use std::io::Write;

fn friendly_string(s: &str) -> String {
    if s.contains([
        ':', '{', '}', '[', ']', ',', '&', '*', '#', '?', '|', '-', '<', '>', '=', '!', '%',
    ]) {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

pub fn journal(notes_dir: &str, contents: Option<Vec<String>>) -> Result<(String, String)> {
    let current_datetime = Local::now();

    let id = format!("j_{}", current_datetime.format("%Y-%m-%d"));
    let title = format!("Journal {}", current_datetime.format("%A %d %B %Y"));

    let path = format!("{notes_dir}/journals/{id}.md");

    let new = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(&title),
        note_type: NoteType::Journal.as_str().to_string(),
        author: None,
        source_parents: None,
        references: None,
        tags: db::List(vec![]),
    };

    write_file(&path, new, contents)?;

    Ok((path, id))
}

pub fn author(
    id: &str,
    title: &str,
    notes_dir: &str,
    contents: Option<Vec<String>>,
) -> Result<(String, String)> {
    let id = format!("a_{id}");
    let path = format!("{notes_dir}/authors/{id}.md");

    let child = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(title),
        note_type: NoteType::Author.as_str().to_string(),
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
    id: &str,
    title: &str,
    notes_dir: &str,
    contents: Option<Vec<String>>,
) -> Result<(String, String)> {
    let path = format!("{notes_dir}/notes/{id}.md");

    let note = Frontmatter {
        id: id.to_string(),
        aliases: friendly_string(title),
        note_type: NoteType::Note.as_str().to_string(),
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
    child_id: &str,
    child_title: &str,
    notes_dir: &str,
    contents: Option<Vec<String>>,
) -> Result<(String, String)> {
    let (id, authors, source_parents) = match NoteType::from_string(&parent.note_type) {
        NoteType::Author => {
            let id = format!("s_{}_{}", parent.id.trim_prefix("a_"), child_id);
            Ok((id, db::List(vec![parent.id]), db::List(vec![])))
        }
        NoteType::Source => {
            let id = format!("{}_{}", parent.id, child_id);
            let authors = parent.author.unwrap_or(db::List(vec![]));
            let mut source_parents = parent.source_parents.unwrap_or(db::List(vec![]));

            source_parents.push(parent.id);

            Ok((id, authors, source_parents))
        }
        _ => Err(MyError::OtherError),
    }?;

    let path = format!("{notes_dir}/sources/{id}.md");

    let child = Frontmatter {
        id: id.clone(),
        aliases: friendly_string(child_title),
        note_type: NoteType::Source.as_str().to_string(),
        author: Some(authors),
        source_parents: Some(source_parents),
        references: parent.references,
        tags: parent.tags,
    };

    write_file(&path, child, contents)?;

    Ok((path, id))
}

fn write_file(path: &str, f: Frontmatter, contents: Option<Vec<String>>) -> Result<()> {
    let mut file = fs::File::create_new(path)?;
    let mut c = f.to_yaml();

    if let Some(mut lines) = contents {
        c.append(&mut lines)
    }

    file.write_all(c.join("\n").as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn journal_creates_markdown_file() {
        let base = std::env::temp_dir().join(format!(
            "mesh-create-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(base.join("journals")).unwrap();
        let notes_dir = base.to_string_lossy().to_string();
        let (path, id) = journal(&notes_dir, None).unwrap();
        assert!(std::path::Path::new(&path).exists());
        assert!(id.starts_with("j_"));
    }

    #[test]
    fn author_creates_markdown_file() {
        let base = std::env::temp_dir().join(format!(
            "mesh-create-author-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(base.join("authors")).unwrap();
        let notes_dir = base.to_string_lossy().to_string();
        let (path, id) = author("doe", "Doe", &notes_dir, None).unwrap();
        assert!(std::path::Path::new(&path).exists());
        assert!(id.starts_with("a_"));
    }
}
