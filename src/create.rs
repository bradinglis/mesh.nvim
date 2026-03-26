use crate::db;
use crate::{MyError, Result, db::Frontmatter};
use std::fs;
use std::io::Write;

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
        aliases: title.to_string(),
        note_type: "author".to_string(),
        author: db::List(vec![]),
        source_parents: db::List(vec![]),
        references: db::List(vec![]),
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
        aliases: title.to_string(),
        note_type: "note".to_string(),
        author: db::List(vec![]),
        source_parents: db::List(vec![]),
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
            let authors = vec![parent.id];
            let source_parents = vec![];
            Ok((id, authors, source_parents))
        }
        "source" => {
            let id = format!("{}_{}", parent.id, &child_id);
            let authors = parent.author.clone();
            let mut source_parents = parent.source_parents.clone();
            source_parents.push(parent.id);

            Ok((id, authors, source_parents))
        }
        _ => Err(MyError::OtherError),
    }?;

    let path = format!("{}/sources/{}.md", notes_dir, id);

    let child = Frontmatter {
        id: id.clone(),
        aliases: child_title.to_string(),
        note_type: "source".to_string(),
        author: db::List(authors),
        source_parents: db::List(source_parents),
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
