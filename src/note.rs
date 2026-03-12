use crate::output;
use crate::{MyError, Result};
use serde::{Deserialize, Serialize};

// pub(crate) use crate::parse::body::BodyParsing;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Note {
    id: String,
    aliases: Option<String>,
    note_type: NoteType,
    tags: Vec<Ref>,
    links: Vec<Ref>,
    contents: Vec<ContentLine>,
    source_parents: Vec<String>,
    author: Vec<String>,
    lines: usize,
    frontmatter_lines: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ContentLine {
    text: String,
    line_num: usize,
    note: String,
}

impl ContentLine {
    pub(crate) fn new(text: String, line_num: usize, note: String) -> Self {
        Self {
            text,
            line_num,
            note,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum NoteType {
    Source,
    Author,
    Note,
}

impl NoteType {
    fn note_type_from_string(s: String) -> Result<Self> {
        match s.as_str() {
            "author" => Ok(Self::Author),
            "source" => Ok(Self::Source),
            "note" => Ok(Self::Note),
            _ => Err(MyError::OtherError),
        }
    }
}

impl Note {
    pub(crate) fn new(
        id: String,
        aliases: Option<String>,
        note_type: NoteType,
        tags: Vec<Ref>,
        links: Vec<Ref>,
        contents: Vec<ContentLine>,
        source_parents: Vec<String>,
        author: Vec<String>,
        lines: usize,
        frontmatter_lines: usize,
    ) -> Self {
        Self {
            id,
            aliases,
            note_type,
            tags,
            links,
            contents,
            source_parents,
            author,
            lines,
            frontmatter_lines,
        }
    }

    fn to_output(self) -> output::Note {
        let note_type = match self.note_type {
            NoteType::Source => "source",
            NoteType::Author => "author",
            NoteType::Note => "note",
        }
        .to_string();

        let tags = self
            .tags
            .iter()
            .map(|x| x.clone().to_output_name())
            .collect();
        let links = self
            .links
            .iter()
            .map(|x| x.clone().to_output_name())
            .collect();

        output::Note {
            id: self.id,
            aliases: self.aliases,
            note_type,
            tags,
            links,
            source_parents: self.source_parents,
            author: self.author,
            lines: self.lines,
            frontmatter_lines: self.frontmatter_lines,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Ref {
    from: RefNode,
    to: RefNode,
    loc: RefLoc,
    alias: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RefNode {
    Note(String),
    Tag(String),
    Local(usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RefLoc {
    FrontMatter,
    Body(usize),
}

impl Ref {
    pub(crate) fn new(from: RefNode, to: RefNode, loc: RefLoc, alias: Option<String>) -> Self {
        Self {
            from,
            to,
            loc,
            alias,
        }
    }

    pub(crate) fn instantiate_intext(mut self, ref_list: &Vec<Ref>) -> Self {
        self.to = match self.to {
            RefNode::Local(y) => ref_list[y].to.clone(),
            z => z,
        };
        self
    }

    fn to_output_name(&self) -> String {
        match self.to.clone() {
            RefNode::Note(y) => y,
            RefNode::Tag(y) => y,
            RefNode::Local(_) => "".to_string(),
        }
    }

    fn to_output_struct(self) -> output::Ref {
        let (ref_type, to) = match self.to {
            RefNode::Note(x) => ("note".to_string(), x),
            RefNode::Tag(x) => ("tag".to_string(), x),
            RefNode::Local(x) => ("local".to_string(), x.to_string()),
        };
        let from = match self.from {
            RefNode::Note(x) => x,
            RefNode::Tag(x) => x,
            RefNode::Local(x) => x.to_string(),
        };
        let line = match self.loc {
            RefLoc::FrontMatter => 0,
            RefLoc::Body(x) => x,
        };

        output::Ref {
            from,
            to,
            ref_type,
            line,
            alias: self.alias,
        }
    }

    pub fn key(&self) -> usize {
        match self.from {
            RefNode::Note(_) => 999,
            RefNode::Tag(_) => 999,
            RefNode::Local(x) => x,
        }
    }

}
