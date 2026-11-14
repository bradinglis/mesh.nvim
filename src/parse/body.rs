use crate::parse::{Lines, frontmatter::FrontMatter};

use crate::{NoteId, Result};
use regex::Regex;
use std::sync::LazyLock;

use crate::note::{Anchor, ContentLine, Note, NoteType, Ref, RefLoc, RefNode};

pub(crate) struct BodyParsing {
    id: Option<String>,
    frontmatter: Option<FrontMatter>,
    contents: Vec<ContentLine>,
    tags: Vec<Ref>,
    links: Vec<Ref>,
    refs: Vec<Ref>,
    ref_list: Vec<Ref>,
    headings: Vec<Anchor>,
    line_num: usize,
}

impl BodyParsing {
    pub(crate) fn init(note: Option<FrontMatter>) -> Result<Self> {
        let line_num = note.as_ref().map_or(1, |x| x.lines);
        Ok(BodyParsing {
            id: note.as_ref().and_then(|n| n.id.to_owned()),
            frontmatter: note,
            contents: vec![],
            tags: vec![],
            links: vec![],
            refs: vec![],
            ref_list: vec![],
            headings: vec![],
            line_num,
        })
    }

    pub(crate) fn consume(mut self) -> Note {
        let mut links = self.links;
        self.ref_list.sort_by_key(Ref::key);

        for reference in self.refs {
            links.push(reference.instantiate_intext(&self.ref_list));
        }

        match self.frontmatter {
            Some(mut frontmatter) => {
                let mut tags = self.tags;
                tags.append(&mut frontmatter.tags);
                links.append(&mut frontmatter.references);

                Note {
                    id: self.id,
                    title: frontmatter.aliases,
                    note_type: frontmatter
                        .note_type
                        .as_deref()
                        .map_or(NoteType::None, NoteType::from_string),
                    tags,
                    links,
                    contents: self.contents,
                    source_parents: frontmatter.source_parents,
                    author: frontmatter.author,
                    lines: self.line_num,
                    headings: self.headings,
                    frontmatter_lines: frontmatter.lines,
                    path: None,
                    hash: None,
                    modified: None,
                }
            }
            None => Note {
                id: self.id,
                title: None,
                note_type: NoteType::None,
                tags: self.tags,
                links,
                contents: self.contents,
                source_parents: vec![],
                author: vec![],
                lines: self.line_num,
                headings: self.headings,
                frontmatter_lines: 0,
                path: None,
                hash: None,
                modified: None,
            },
        }
    }

    pub(crate) fn parse_lines(mut self, rest: Lines) -> Self {
        rest.for_each(|line| self.parse_line(line));
        self
    }

    fn parse_line(&mut self, text: Result<String>) {
        let text = text.unwrap_or_default();
        self.line_num += 1;

        // Cheap character guards keep the regexes off the common path.
        if text.contains('#') {
            self.get_tags(&text);
            if text.starts_with('#') {
                self.get_headings(&text);
            }
        }
        if text.contains("[[") {
            self.get_links(&text);
        }
        if text.contains("][") {
            self.get_reference_intext(&text);
        }
        if text.starts_with('[') {
            self.get_reference_list(&text);
        }

        self.contents
            .push(ContentLine::new(text, self.line_num, self.id.clone()));
    }

    fn get_tags(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s#([a-zA-Z\-]+)").unwrap());

        for (range, tag) in RE.captures_iter(line).filter_map(|c| {
            let tag = c.get(1)?.as_str().to_string();
            Some((c.get(0)?.range(), tag))
        }) {
            self.tags.push(Ref::new(
                self.id.clone().map(RefNode::NoteAnchor),
                RefNode::Tag(tag),
                RefLoc::Body(self.line_num, range.start + 1, range.end),
                None,
            ));
        }
    }

    fn get_headings(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#+) (.+)$").unwrap());

        for (range, prefix, heading) in RE
            .captures_iter(line)
            .filter_map(|c| Some((c.get(0)?.range(), c.get(1)?.as_str(), c.get(2)?.as_str())))
        {
            let level = prefix.len();
            let heading_link = if level == 1 {
                self.id
                    .clone()
                    .unwrap_or_else(|| NoteId::derive(heading).to_string())
            } else {
                let mut link = match self.parent_heading(level - 1) {
                    Some(parent) => format!("{parent}_"),
                    None => "#".to_string(),
                };
                link.push_str(&NoteId::derive_subheading(heading).to_string());
                link
            };

            self.headings.push(Anchor::new(
                self.id.to_owned(),
                heading.to_string(),
                heading_link,
                RefLoc::Body(self.line_num, range.start, range.end),
                level,
            ));
        }
    }

    fn parent_heading(&self, level: usize) -> Option<&str> {
        self.headings
            .iter()
            .rev()
            .find(|anchor| anchor.level == level)
            .map(|anchor| anchor.heading_link.as_str())
    }

    fn get_links(&mut self, line: &str) {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[\[([^\|\]]+)\|?([^\|\]]*)\]\]").unwrap());

        for (range, to_id, title) in RE
            .captures_iter(line)
            .filter_map(|c| Some((c.get(0)?.range(), c.get(1)?.as_str(), c.get(2)?.as_str())))
        {
            self.links.push(Ref::new(
                self.id.clone().map(RefNode::NoteAnchor),
                RefNode::NoteAnchor(to_id.to_owned()),
                RefLoc::Body(self.line_num, range.start, range.end),
                Some(title.to_string()),
            ));
        }
    }

    fn get_reference_intext(&mut self, line: &str) {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([0-9]+)\]").unwrap());

        for (range, name, ref_num) in RE
            .captures_iter(line)
            .filter_map(|c| Some((c.get(0)?.range(), c.get(1)?.as_str(), c.get(2)?.as_str())))
        {
            self.refs.push(Ref::new(
                self.id.clone().map(RefNode::NoteAnchor),
                RefNode::Local(ref_num.parse().unwrap_or(0)),
                RefLoc::Body(self.line_num, range.start, range.end),
                Some(name.to_string()),
            ));
        }
    }

    fn get_reference_list(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[([0-9]+)\]: (.+)").unwrap());

        for (range, ref_num, to_id) in RE
            .captures_iter(line)
            .filter_map(|c| Some((c.get(0)?.range(), c.get(1)?.as_str(), c.get(2)?.as_str())))
        {
            self.ref_list.push(Ref::new(
                ref_num.parse().ok().map(RefNode::Local),
                RefNode::NoteAnchor(to_id.to_owned()),
                RefLoc::Body(self.line_num, range.start, range.end),
                None,
            ));
        }
    }
}
