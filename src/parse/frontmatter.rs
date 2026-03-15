use std::path::PathBuf;
use std::path::Path;
use std::str::SplitWhitespace;

use crate::MyError;
use crate::Result;
use crate::fold_rest::{FoldRest, IterState};
use crate::note::{NoteType, Ref, RefLoc, RefNode};
use crate::parse::LinesIter;
use crate::parse::body::InterimNote;

#[derive(Debug)]
pub(crate) struct FrontMatterParsing {
    id: Option<String>,
    aliases: Option<String>,
    note_type: Option<String>,
    tags: Vec<Ref>,
    references: Vec<Ref>,
    source_parents: Vec<String>,
    author: Vec<String>,
}

impl FrontMatterParsing {
    pub fn parse(path: &Path, iter: LinesIter) -> Result<InterimNote> {
        let mut new = Self::new(path);

        let mut line_num: usize = 0;
        let rest = iter.for_each_rest(|x| {
            let line = x?;
            line_num += 0;
            match line.as_str() {
                "---" => Ok(IterState::Break),
                _ => {
                    new.add_line_to_frontmatter(line)?;
                    Ok(IterState::Continue)
                }
            }
        })?;

        let id = new.id.ok_or(MyError::ParseError("id not correct".to_string()))?;
        let note_type = new.note_type.ok_or(MyError::OtherError)?;
        Ok(InterimNote::new(
            id,
            new.aliases,
            NoteType::from_string(note_type)?,
            new.tags,
            new.references,
            new.source_parents,
            new.author,
            line_num,
            Some(rest),
            path.to_path_buf(),
        ))
    }

    fn add_line_to_frontmatter(&mut self, line: String) -> Result<()> {
        let _: () = match line.split_once(": ") {
            Some(("id", suff)) => self.id = Some(suff.to_string()),
            Some(("aliases", suff)) => self.aliases = Some(suff.to_string()),
            Some(("type", suff)) => self.note_type = Some(suff.to_string()),
            Some(("source-parents", suff)) => self.add_source_parents(suff),
            Some(("author", suff)) => self.add_authors(suff),
            Some(("references", suff)) => self.add_references(suff),
            Some(("tags", suff)) => self.add_tags(suff),
            _ => Err(MyError::OtherError)?,
        };
        Ok(())
    }

    fn add_source_parents(&mut self, text: &str) {
        if let Some(parents) = Self::get_text_list(text).map(|iter| iter.map(|parent| parent.to_string()).collect()) { self.source_parents = parents }
    }

    fn add_authors(&mut self, text: &str) {
        if let Some(authors) = Self::get_text_list(text).map(|iter| iter.map(|author| author.to_string()).collect()) {
            self.author = authors
        }
    }

    fn add_tags(&mut self, text: &str) {
        if let Some(tags) = Self::get_text_list(text).map(|iter| {
            iter.map(|tag| {
                Ref::new(
                    RefNode::Note(self.id.clone().unwrap_or("".to_string())),
                    RefNode::Tag(tag.to_string()),
                    RefLoc::FrontMatter,
                    None,
                )
            })
            .collect()
        }) { self.tags = tags }
    }

    fn add_references(&mut self, text: &str) {
        if let Some(references) = Self::get_text_list(text).map(|iter| {
            iter.map(|reference| {
                Ref::new(
                    RefNode::Note(self.id.clone().unwrap_or("".to_string())),
                    RefNode::Note(reference.to_string()),
                    RefLoc::FrontMatter,
                    None,
                )
            })
            .collect()
        }) { self.references = references }
    }

    fn get_text_list(text: &str) -> Option<SplitWhitespace<'_>> {
        match text {
            "[]" => None,
            y => Some(y.split_whitespace()),
        }
    }

    fn new(path: &Path) -> Self {
        Self {
            id: None,
            aliases: None,
            note_type: None,
            tags: vec![],
            references: vec![],
            source_parents: vec![],
            author: vec![],
        }
    }
}

