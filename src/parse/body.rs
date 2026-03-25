use crate::parse::LinesIter;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::{MyError, Result, ToNoteId};
use regex::Regex;
use std::sync::LazyLock;

use crate::note::{Anchor, ContentLine, Note, NoteType, Ref, RefLoc, RefNode};

pub(crate) struct BodyParsing {
    id: String,
    contents: Vec<ContentLine>,
    tags: Vec<Ref>,
    links: Vec<Ref>,
    refs: Vec<Ref>,
    ref_list: Vec<Ref>,
    line_num: usize,
    aliases: Option<String>,
    note_type: NoteType,
    references: Vec<Ref>,
    source_parents: Vec<Ref>,
    author: Vec<Ref>,
    headings: Vec<Anchor>,
    frontmatter_lines: usize,
}

pub struct InterimNote {
    pub(super) id: String,
    pub(super) aliases: Option<String>,
    pub(super) note_type: NoteType,
    pub(super) tags: Vec<Ref>,
    pub(super) references: Vec<Ref>,
    pub(super) source_parents: Vec<Ref>,
    pub(super) author: Vec<Ref>,
    pub(super) frontmatter_lines: usize,
    pub(super) rest: Option<LinesIter>,
}

impl BodyParsing {
    pub(crate) fn init(note: InterimNote) -> Result<(Self, LinesIter)> {
        let rest = note
            .rest
            .ok_or(MyError::ParseError("reader not working".to_string()))?;
        Ok((
            BodyParsing {
                id: note.id,
                contents: vec![],
                tags: note.tags,
                links: vec![],
                line_num: note.frontmatter_lines,
                aliases: note.aliases,
                note_type: note.note_type,
                refs: vec![],
                headings: vec![],
                ref_list: vec![],
                references: note.references,
                source_parents: note.source_parents,
                author: note.author,
                frontmatter_lines: note.frontmatter_lines,
            },
            rest,
        ))
    }

    pub(crate) fn consume(
        mut self,
        path: Option<PathBuf>,
        hash: Option<u64>,
        modified: Option<SystemTime>,
    ) -> Note {
        self.ref_list.sort_by_key(|x| x.key());

        self.refs.into_iter().for_each(|x| {
            let x = x.instantiate_intext(&self.ref_list);
            self.links.push(x);
        });

        self.links.append(&mut self.references);

        Note {
            id: self.id,
            title: self.aliases,
            note_type: self.note_type,
            tags: self.tags,
            links: self.links,
            contents: self.contents,
            source_parents: self.source_parents,
            author: self.author,
            lines: self.line_num,
            headings: self.headings,
            frontmatter_lines: self.frontmatter_lines,
            path,
            hash,
            modified,
        }
    }

    pub(crate) fn parse_lines(&mut self, rest: LinesIter) {
        rest.for_each(|x| self.parse_line(x));
    }

    fn parse_line(&mut self, text: Result<String>) {
        let text = text.unwrap_or("".to_string());
        self.get_tags(&text);
        self.get_links(&text);
        self.get_reference_intext(&text);
        self.get_reference_list(&text);
        self.get_headings(&text);
        self.contents
            .push(ContentLine::new(text, self.line_num, self.id.clone()));
        self.line_num += 1;
    }

    fn get_tags(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s#([a-zA-Z\-]+)").unwrap());

        for (r, (_, [tag])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            self.tags.push(Ref::new(
                RefNode::NoteAnchor(self.id.to_string()),
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
            let mut heading_link = self.id.to_string();

            let level = prefix.len();
            // Possibly do heading level stuff here.. and subheading handling??
            match level {
                1 => (),
                2 => {
                    heading_link.push('#');
                    heading_link.push_str(&heading.to_string().to_note_id());
                }
                x => {
                    if let Some(x) = self.parent_anchor(x).map(|x| x.heading_link) {
                        heading_link = x;
                        heading_link.push('_');
                    } else {
                        heading_link.push('#');
                    }
                    heading_link.push_str(&heading.to_string().to_note_id());
                }
            }

            self.headings.push(Anchor::new(
                self.id.to_string(),
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
                RefNode::NoteAnchor(self.id.to_string()),
                RefNode::NoteAnchor(to_id.to_string()),
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
                RefNode::NoteAnchor(self.id.to_string()),
                RefNode::Local(ref_num.parse().unwrap_or(0)),
                RefLoc::Body(self.line_num, r.start, r.end),
                Some(name.to_string()),
            ));
        }
    }

    fn get_reference_list(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([0-9]+)\]: (.+)").unwrap());

        for (r, (_, [ref_num, to_id])) in RE
            .captures_iter(line)
            .map(|c| (c.get_match().range(), c.extract()))
        {
            self.ref_list.push(Ref::new(
                RefNode::Local(ref_num.parse().unwrap_or(0)),
                RefNode::NoteAnchor(to_id.to_string()),
                RefLoc::Body(self.line_num, r.start, r.end),
                None,
            ));
        }
    }
}
