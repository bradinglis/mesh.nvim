use crate::parse::{Lines, frontmatter::FrontMatter};

use crate::{Result, NoteId};
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

    pub(crate) fn consume(
        mut self,
    ) -> Note {
        let mut links = self.links;
        self.ref_list.sort_by_key(|x| x.key());

        self.refs.into_iter().for_each(|x| {
            let x = x.instantiate_intext(&self.ref_list);
            links.push(x);
        });

        match self.frontmatter {
            Some(f) => {
                let mut tags = self.tags;
                tags.append(&mut f.tags.clone());
                links.append(&mut f.references.clone());

                Note {
                    id: self.id,
                    title: f.aliases,
                    note_type: f.note_type.map_or(NoteType::None, NoteType::from_string),
                    tags,
                    links,
                    contents: self.contents,
                    source_parents: f.source_parents,
                    author: f.author,
                    lines: self.line_num,
                    headings: self.headings,
                    frontmatter_lines: f.lines,
                    path: None,
                    hash: None,
                    modified: None,
                }
            },
            None => {
                Note {
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
                }
            },
        }
    }

    pub(crate) fn parse_lines(mut self, rest: Lines) -> Self {
        rest.for_each(|x| self.parse_line(x));
        self
    }

    fn parse_line(&mut self, text: Result<String>) {
        let text = text.unwrap_or("".to_string());
        self.line_num += 1;
        self.get_tags(&text);
        self.get_links(&text);
        self.get_reference_intext(&text);
        self.get_reference_list(&text);
        self.get_headings(&text);
        self.contents
            .push(ContentLine::new(text, self.line_num, self.id.clone()));
    }

    fn get_tags(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s#([a-zA-Z\-]+)").unwrap());

        for (r, (_, [tag])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            self.tags.push(Ref::new(
                self.id.clone().map(RefNode::NoteAnchor),
                RefNode::Tag(tag.to_string()),
                RefLoc::Body(self.line_num, r.start + 1, r.end),
                None,
            ));
        }
    }

    fn get_headings(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#+) (.+)$").unwrap());

        for (r, (_, [prefix, heading])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            let mut heading_link = String::new();

            let level = prefix.len();
            // Possibly do heading level stuff here.. and subheading handling??
            match level {
                1 => {
                    if let Some(x) = self.id.clone() {
                        heading_link.push_str(x.as_str());
                    } else {

                        heading_link.push_str(&NoteId::derive(heading).to_owned());
                    }
                },
                2 => {
                    if let Some(x) = self.parent_anchor(1).map(|x| x.heading_link) {
                        heading_link = x;
                        heading_link.push('_');
                    } else {
                        heading_link.push('#');
                    }
                    heading_link.push_str(&NoteId::derive(heading).to_owned());
                },
                x => {
                    if let Some(x) = self.parent_anchor(x - 1).map(|x| x.heading_link) {
                        heading_link = x;
                        heading_link.push('_');
                    } else {
                        heading_link.push('#');
                    }
                    heading_link.push_str(&NoteId::derive(heading).to_owned());
                }
            }

            self.headings.push(Anchor::new(
                self.id.to_owned(),
                heading.to_string(),
                heading_link,
                RefLoc::Body(self.line_num, r.start, r.end),
                level,
            ));
        }
    }

    fn parent_anchor(&self, level: usize) -> Option<Anchor> {
        self.headings
            .clone()
            .into_iter()
            .rfind(|x| x.level == level)
    }

    fn get_links(&mut self, line: &str) {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[\[([^\|\]]+)\|?([^\|\]]*)\]\]").unwrap());

        for (r, (_, [to_id, title])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            self.links.push(Ref::new(
                self.id.clone().map(RefNode::NoteAnchor),
                RefNode::NoteAnchor(to_id.to_owned()),
                RefLoc::Body(self.line_num, r.start, r.end),
                Some(title.to_string()),
            ));
        }
    }

    fn get_reference_intext(&mut self, line: &str) {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([0-9]+)\]").unwrap());

        for (r, (_, [name, ref_num])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            self.refs.push(Ref::new(
                self.id.clone().map(RefNode::NoteAnchor),
                RefNode::Local(ref_num.parse().unwrap_or(0)),
                RefLoc::Body(self.line_num, r.start, r.end),
                Some(name.to_string()),
            ));
        }
    }

    fn get_reference_list(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[([0-9]+)\]: (.+)").unwrap());

        for (r, (_, [ref_num, to_id])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            self.ref_list.push(Ref::new(
                ref_num.parse().ok().map(RefNode::Local),
                RefNode::NoteAnchor(to_id.to_owned()),
                RefLoc::Body(self.line_num, r.start, r.end),
                None,
            ));
        }
    }
}
