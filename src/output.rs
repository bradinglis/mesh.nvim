
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Note {
    pub(crate) id: String,
    pub(crate) aliases: Option<String>,
    pub(crate) note_type: String,
    pub(crate) tags: Vec<String>,
    pub(crate) links: Vec<String>,
    pub(crate) source_parents: Vec<String>,
    pub(crate) author: Vec<String>,
    pub(crate) lines: usize,
    pub(crate) frontmatter_lines: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Ref {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) ref_type: String,
    pub(crate) line: usize,
    pub(crate) alias: Option<String>,
}



