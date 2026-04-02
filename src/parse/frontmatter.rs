use std::iter::Peekable;
use std::path::Path;
use std::str::SplitWhitespace;

use crate::MyError;
use crate::Result;
use crate::fold_rest::{FoldRest, IterState};
use crate::note::{NoteType, Ref, RefLoc, RefNode};
use crate::parse::LinesIter;
use crate::parse::body::InterimNote;

#[derive(Debug)]
pub(crate) struct FrontMatter {
    id: Option<String>,
    aliases: Option<String>,
    note_type: Option<String>,
    tags: Vec<Ref>,
    references: Vec<Ref>,
    source_parents: Vec<Ref>,
    author: Vec<Ref>,
    lines: usize,
}

impl FrontMatter {
    pub fn parse(iter: LinesIter) -> Result<(Option<FrontMatter>, Peekable<LinesIter>)> {
        let mut iter = iter.peekable();
        if let Some(Ok(l)) = iter.peek() && l == "---" {
            let mut new = Self::new();
            let rest = iter.for_each_rest(|x| {
                let line = x?;
                new.lines += 1;
                match line.as_str() {
                    "---" => Ok(IterState::Break),
                    _ => {
                        new.add_line_to_frontmatter(line, new.lines)?;
                        Ok(IterState::Continue)
                    }
                }
            })?;
            Ok((None, iter))
        } else {
            Ok((None, iter))
        }
        // lnum += 1;
        //
        // let id = new
        //     .id
        //     .ok_or(MyError::ParseError("id not correct".to_string()))?;
        // let note_type = new.note_type.unwrap_or("".to_string());
        //
        // Ok(InterimNote {
        //     id,
        //     aliases: new.aliases,
        //     note_type: NoteType::from_string(note_type)?,
        //     tags: new.tags,
        //     references: new.references,
        //     source_parents: new.source_parents,
        //     author: new.author,
        //     rest: Some(rest),
        //     frontmatter_lines: lnum,
        // })
    }

    fn add_line_to_frontmatter(&mut self, line: String, lnum: usize) -> Result<()> {
        let _: () = match line.split_once(": ") {
            Some(("id", suff)) => self.id = Some(suff.to_string()),
            Some(("aliases", suff)) => self.aliases = Some(suff.to_string()),
            Some(("type", suff)) => self.note_type = Some(suff.to_string()),
            Some(("source-parents", suff)) => self.add_source_parents(suff, lnum),
            Some(("author", suff)) => self.add_authors(suff, lnum),
            Some(("references", suff)) => self.add_references(suff, lnum),
            Some(("tags", suff)) => self.add_tags(suff, lnum),
            Some((x, y)) => Err(MyError::ParseError(format!("Unknown field: {x}: {y}")))?,
            None => Err(MyError::ParseError("Unknown field".to_string()))?,
        };
        Ok(())
    }

    fn add_source_parents(&mut self, text: &str, line: usize) {
        let mut pos: usize = 16;
        if let Some(parents) = Self::get_text_list(text).map(|iter| {
            iter.map(|parent| {
                let start = pos;
                let end = pos + parent.len();
                pos = end + 1;
                Ref::new(
                    RefNode::NoteAnchor(self.id.clone().unwrap_or("".to_string())),
                    RefNode::NoteAnchor(parent.to_string()),
                    RefLoc::FrontMatter(line, start, end),
                    None,
                )
            })
            .collect()
        }) {
            self.source_parents = parents
        }
    }

    fn add_authors(&mut self, text: &str, line: usize) {
        let mut pos: usize = 8;
        if let Some(authors) = Self::get_text_list(text).map(|iter| {
            iter.map(|author| {
                let start = pos;
                let end = pos + author.len();
                pos = end + 1;
                Ref::new(
                    RefNode::NoteAnchor(self.id.clone().unwrap_or("".to_string())),
                    RefNode::NoteAnchor(author.to_string()),
                    RefLoc::FrontMatter(line, start, end),
                    None,
                )
            })
            .collect()
        }) {
            self.author = authors
        }
    }

    fn add_tags(&mut self, text: &str, line: usize) {
        let mut pos: usize = 6;
        if let Some(tags) = Self::get_text_list(text).map(|iter| {
            iter.map(|tag| {
                let start = pos;
                let end = pos + tag.len();
                pos = end + 1;
                Ref::new(
                    RefNode::NoteAnchor(self.id.clone().unwrap_or("".to_string())),
                    RefNode::Tag(tag.to_string()),
                    RefLoc::FrontMatter(line, start, end),
                    None,
                )
            })
            .collect()
        }) {
            self.tags = tags
        }
    }

    fn add_references(&mut self, text: &str, line: usize) {
        let mut pos: usize = 12;
        if let Some(references) = Self::get_text_list(text).map(|iter| {
            iter.map(|reference| {
                let start = pos;
                let end = pos + reference.len();
                pos = end + 1;
                Ref::new(
                    RefNode::NoteAnchor(self.id.clone().unwrap_or("".to_string())),
                    RefNode::NoteAnchor(reference.to_string()),
                    RefLoc::FrontMatter(line, start, end),
                    None,
                )
            })
            .collect()
        }) {
            self.references = references
        }
    }

    fn get_text_list(text: &str) -> Option<SplitWhitespace<'_>> {
        match text {
            "[]" => None,
            y => Some(y.split_whitespace()),
        }
    }

    fn new() -> Self {
        Self {
            id: None,
            aliases: None,
            note_type: None,
            tags: vec![],
            references: vec![],
            source_parents: vec![],
            author: vec![],
            lines: 1,
        }
    }
}
