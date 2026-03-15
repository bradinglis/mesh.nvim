use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::output;
use crate::{MyError, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Note {
    pub(crate) id: String,
    pub(crate) aliases: Option<String>,
    pub(crate) note_type: NoteType,
    pub(crate) tags: Vec<Ref>,
    pub(crate) links: Vec<Ref>,
    pub(crate) contents: Vec<ContentLine>,
    pub(crate) source_parents: Vec<String>,
    pub(crate) author: Vec<String>,
    pub(crate) lines: usize,
    pub(crate) frontmatter_lines: usize,
    pub(crate) path: PathBuf,
    pub(crate) hash: u64,
    pub(crate) modified: SystemTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ContentLine {
    pub(crate) note_id: String,
    pub(crate) text: String,
    pub(crate) line_num: usize,
}

impl ContentLine {
    pub(crate) fn new(text: String, line_num: usize, note_id: String) -> Self {
        Self {
            text,
            line_num,
            note_id,
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
    pub fn from_string(s: String) -> Result<Self> {
        match s.as_str() {
            "author" => Ok(Self::Author),
            "source" => Ok(Self::Source),
            "note" => Ok(Self::Note),
            _ => Err(MyError::ParseError("note type unknown".to_string())),
        }
    }

    pub fn to_string(self) -> String {
        match self {
            NoteType::Source => "source".to_string(),
            NoteType::Author => "author".to_string(),
            NoteType::Note => "note".to_string(),
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
        path: PathBuf,
        hash: u64,
        modified: SystemTime,
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
            path,
            hash,
            modified
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
    pub(crate) from: RefNode,
    pub(crate) to: RefNode,
    pub(crate) loc: RefLoc,
    pub(crate) alias: Option<String>,
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

    pub(crate) fn get_tag(&self) -> Result<String> {
        if let RefNode::Tag(x) = &self.to {
            Ok(x.clone())
        } else {
            Err(MyError::OtherError)
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
