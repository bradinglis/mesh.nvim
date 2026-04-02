use std::{fmt::Display, ops::Deref};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::{MyError, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Note {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) note_type: NoteType,
    pub(crate) tags: Vec<Ref>,
    pub(crate) links: Vec<Ref>,
    pub(crate) headings: Vec<Anchor>,
    pub(crate) contents: Vec<ContentLine>,
    pub(crate) source_parents: Vec<Ref>,
    pub(crate) author: Vec<Ref>,
    pub(crate) lines: usize,
    pub(crate) frontmatter_lines: usize,
    pub(crate) path: Option<PathBuf>,
    pub(crate) hash: Option<u64>,
    pub(crate) modified: Option<SystemTime>,
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
    Journal,
}

impl NoteType {
    pub fn from_string(s: String) -> Result<Self> {
        match s.as_str() {
            "author" => Ok(Self::Author),
            "source" => Ok(Self::Source),
            "note" => Ok(Self::Note),
            "journal" => Ok(Self::Journal),
            _ => Err(MyError::ParseError("note type unknown".to_string())),
        }
    }
}

impl Display for NoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let x = match self {
            NoteType::Source => "source".to_string(),
            NoteType::Author => "author".to_string(),
            NoteType::Note => "note".to_string(),
            NoteType::Journal => "journal".to_string(),
        };

        f.write_fmt(format_args!("{}", x))
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
pub struct Anchor {
    pub(crate) note_id: String,
    pub(crate) title: String,
    pub(crate) heading_link: String,
    pub(crate) loc: RefLoc,
    pub(crate) level: usize,
}

impl Anchor {
    pub fn new(note_id: String, title: String, heading_link: String, loc: RefLoc, level: usize) -> Self {
        Self { note_id, title, heading_link, loc, level }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RefNode {
    NoteAnchor(String),
    Tag(String),
    Local(usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RefLoc {
    FrontMatter(usize, usize, usize),
    Body(usize, usize, usize),
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

    pub(crate) fn instantiate_intext(mut self, ref_list: &[Ref]) -> Self {
        self.to = match self.to {
            RefNode::Local(y) => ref_list[y].to.clone(),
            z => z,
        };
        self
    }

    pub fn key(&self) -> usize {
        match self.from {
            RefNode::NoteAnchor(_) => 999,
            RefNode::Tag(_) => 999,
            RefNode::Local(x) => x,
        }
    }

}

impl Note {
    pub fn get_ref(&self, col: usize, row: usize) -> Option<Ref> {
        let mut refs = self.tags.clone();
        refs.append(&mut self.links.clone());
        refs.append(&mut self.source_parents.clone());
        refs.append(&mut self.author.clone());

        refs.into_iter().find(|r| {
            let (x, y, z) = r.loc.clone().get_loc();
            (x == col) && (y <= row) && (z >= row)
        })
    }
}

impl RefLoc {
    pub fn get_loc(self) -> (usize, usize, usize) {
        match self {
            RefLoc::FrontMatter(x, y, z) => (x, y, z),
            RefLoc::Body(x, y, z) => (x, y, z),
        }
    }
}
