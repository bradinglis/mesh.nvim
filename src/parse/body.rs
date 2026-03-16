use crate::parse::LinesIter;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::{MyError, Result};
use regex::Regex;
use std::io::{self};
use std::sync::LazyLock;

use crate::note::{ContentLine, Note, NoteType, Ref, RefLoc, RefNode};

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
    source_parents: Vec<String>,
    author: Vec<String>,
    frontmatter_lines: usize,
    path: PathBuf,
}

#[derive(Debug)]
pub struct InterimNote {
    pub(super) id: String,
    pub(super) aliases: Option<String>,
    pub(super) note_type: NoteType,
    pub(super) tags: Vec<Ref>,
    pub(super) references: Vec<Ref>,
    pub(super) source_parents: Vec<String>,
    pub(super) author: Vec<String>,
    pub(super) frontmatter_lines: usize,
    pub(super) rest: Option<LinesIter>,
    pub(super) path: PathBuf,
}

impl BodyParsing {
    pub(crate) fn init(note: InterimNote) -> Result<(Self, LinesIter)> {
        let rest =  note.rest.ok_or(MyError::ParseError("reader not working".to_string()))?;
        Ok((BodyParsing {
            id: note.id,
            contents: vec![],
            tags: note.tags,
            links: vec![],
            line_num: note.frontmatter_lines,
            aliases: note.aliases,
            note_type: note.note_type,
            refs: vec![],
            ref_list: vec![],
            references: note.references,
            source_parents: note.source_parents,
            author: note.author,
            frontmatter_lines: note.frontmatter_lines,
            path: note.path,
        }, rest))
    }

    pub(crate) fn consume(mut self, hash: u64, modified: SystemTime) -> Note {
        self.ref_list.sort_by_key(|x| x.key());

        self.refs.into_iter().for_each(|x| {
            let x = x.instantiate_intext(&self.ref_list);
            self.links.push(x);
        });

        self.links.append(&mut self.references);

        Note {
            id: self.id,
            aliases: self.aliases,
            note_type: self.note_type,
            tags: self.tags,
            links: self.links,
            contents: self.contents,
            source_parents: self.source_parents,
            author: self.author,
            lines: self.line_num,
            frontmatter_lines: self.frontmatter_lines,
            path: self.path,
            hash,
            modified
        }
    }

    pub(crate) fn parse_lines(&mut self, rest: LinesIter) {
        rest.for_each(|x| self.parse_line(x));
    }

    fn parse_line(&mut self, text: std::result::Result<String, io::Error>) {
        let text = text.unwrap_or("".to_string());
        self.get_tags(&text);
        self.get_links(&text);
        self.get_reference_intext(&text);
        self.get_reference_list(&text);
        self.contents
            .push(ContentLine::new(text, self.line_num, self.id.clone()));
        self.line_num += 1;
    }

    fn get_tags(&mut self, line: &str) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#([a-zA-Z\-]+)").unwrap());

        for (_, [tag]) in RE.captures_iter(line).map(|c| c.extract()) {
            self.tags.push(Ref::new(
                RefNode::Note(self.id.to_string()),
                RefNode::Tag(tag.to_string()),
                RefLoc::Body(self.line_num),
                None,
            ));
        }
    }

    fn get_links(&mut self, line: &str) {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[\[([^\|\]]+)\|?([^\|\]]*)\]\]").unwrap());

        for (_, [to_id, title]) in RE.captures_iter(line).map(|c| c.extract()) {
            self.links.push(Ref::new(
                RefNode::Note(self.id.to_string()),
                RefNode::Note(to_id.to_string()),
                RefLoc::Body(self.line_num),
                Some(title.to_string()),
            ));
        }
    }

    fn get_reference_intext(&mut self, line: &str) {
        static RE_INTEXT: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([0-9]+)\]").unwrap());
        for (_, [name, ref_num]) in RE_INTEXT.captures_iter(line).map(|c| c.extract()) {
            self.refs.push(Ref::new(
                RefNode::Note(self.id.to_string()),
                RefNode::Local(ref_num.parse().unwrap_or(0)),
                RefLoc::Body(self.line_num),
                Some(name.to_string()),
            ));
        }
    }

    fn get_reference_list(&mut self, line: &str) {
        static RE_REF: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[([0-9]+)\]: (.+)").unwrap());
        for (_, [ref_num, to_id]) in RE_REF.captures_iter(line).map(|c| c.extract()) {
            self.ref_list.push(Ref::new(
                RefNode::Local(ref_num.parse().unwrap_or(0)),
                RefNode::Note(to_id.to_string()),
                RefLoc::Body(self.line_num),
                None,
            ));
        }
    }
}
