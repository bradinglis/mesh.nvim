use std::str::SplitWhitespace;

use crate::Result;
use crate::fold_rest::{IterState, for_each_rest};
use crate::note::{Ref, RefLoc, RefNode};
use crate::parse::Lines;

#[derive(Debug)]
pub(crate) struct FrontMatter {
    pub(crate) id: Option<String>,
    pub(crate) aliases: Option<String>,
    pub(crate) note_type: Option<String>,
    pub(crate) tags: Vec<Ref>,
    pub(crate) references: Vec<Ref>,
    pub(crate) source_parents: Vec<Ref>,
    pub(crate) author: Vec<Ref>,
    pub(crate) lines: usize,
}

impl FrontMatter {
    pub fn parse(mut iter: Lines) -> Result<(Option<FrontMatter>, Lines)> {
        if let Some(Ok(line)) = iter.peek()
            && line == "---"
        {
            let _ = iter.next();
            let mut frontmatter = Self::new();
            let rest = for_each_rest(iter, |line| {
                let line = line?;
                frontmatter.lines += 1;
                match line.as_str() {
                    "---" => Ok(IterState::Break),
                    _ => {
                        frontmatter.add_line_to_frontmatter(&line);
                        Ok(IterState::Continue)
                    }
                }
            })?;
            Ok((Some(frontmatter), rest))
        } else {
            Ok((None, iter))
        }
    }

    // Unknown or malformed frontmatter fields are ignored, matching the
    // parser's tolerant handling of notes from other tools.
    fn add_line_to_frontmatter(&mut self, line: &str) {
        let Some((key, value)) = line.split_once(": ") else {
            return;
        };
        let line_num = self.lines;
        let start = key.len() + 2;

        match key {
            "id" => self.id = Some(value.to_string()),
            "aliases" => self.aliases = Some(value.trim_matches('"').to_string()),
            "type" => self.note_type = Some(value.to_string()),
            "source-parents" => {
                self.source_parents = self.parse_ref_list(value, line_num, start, false)
            }
            "author" => self.author = self.parse_ref_list(value, line_num, start, false),
            "references" => self.references = self.parse_ref_list(value, line_num, start, false),
            "tags" => self.tags = self.parse_ref_list(value, line_num, start, true),
            _ => {}
        }
    }

    fn parse_ref_list(&self, text: &str, line: usize, start: usize, tag: bool) -> Vec<Ref> {
        let Some(items) = Self::get_text_list(text) else {
            return Vec::new();
        };

        let mut pos = start;
        items
            .map(|item| {
                let end = pos + item.len();
                let to = if tag {
                    RefNode::Tag(item.to_string())
                } else {
                    RefNode::NoteAnchor(item.to_string())
                };
                let reference = Ref::new(
                    Some(RefNode::NoteAnchor(self.id.clone().unwrap_or_default())),
                    to,
                    RefLoc::FrontMatter(line, pos, end),
                    None,
                );
                pos = end + 1;
                reference
            })
            .collect()
    }

    fn get_text_list(text: &str) -> Option<SplitWhitespace<'_>> {
        match text {
            "[]" => None,
            value => Some(value.split_whitespace()),
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
