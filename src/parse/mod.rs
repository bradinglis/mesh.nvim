use crate::Result;
use crate::note;
use std::{fs, io, iter::Peekable};

pub(crate) mod body;
pub(crate) mod frontmatter;

// type LinesIter = io::Lines<io::BufReader<fs::File>>;
pub type Lines = Peekable<LinesIter>;

pub enum LinesIter {
    File(io::Lines<io::BufReader<fs::File>>),
    List(std::vec::IntoIter<String>),
}

impl Iterator for LinesIter {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            LinesIter::File(lines) => lines.next().map(|x| x.map_err(|x| x.into())),
            LinesIter::List(items) => items.next().map(Ok),
        }
    }
}

pub(crate) fn parse_note(lines: LinesIter) -> Result<note::Note> {
    let lines: Lines = lines.peekable();
    let (x, y) = frontmatter::FrontMatter::parse(lines)?;
    Ok(body::BodyParsing::init(x)?.parse_lines(y).consume())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::NoteType;

    #[test]
    fn parses_frontmatter_and_body() {
        let lines = vec![
            "---".to_string(),
            "id: test-id".to_string(),
            "aliases: Test".to_string(),
            "type: note".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
            "# Heading".to_string(),
            "Hello [[Other]] #tag".to_string(),
        ];
        let note = parse_note(LinesIter::List(lines.into_iter())).unwrap();
        assert_eq!(note.id.as_deref(), Some("test-id"));
        assert_eq!(note.title.as_deref(), Some("Test"));
        assert_eq!(note.note_type, NoteType::Note);
        assert!(!note.tags.is_empty());
    }

    #[test]
    fn parses_note_without_frontmatter() {
        let lines = vec!["Hello world".to_string()];
        let note = parse_note(LinesIter::List(lines.into_iter())).unwrap();
        assert!(note.id.is_none());
        assert_eq!(note.note_type, NoteType::None);
    }
}
