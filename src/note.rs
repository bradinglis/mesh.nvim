use std::fmt::Display;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub(crate) struct Note {
    pub(crate) id: Option<String>,
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

#[derive(Clone, Debug)]
pub(crate) struct ContentLine {
    pub(crate) note_id: Option<String>,
    pub(crate) text: String,
    pub(crate) line_num: usize,
}

impl ContentLine {
    pub(crate) fn new(text: String, line_num: usize, note_id: Option<String>) -> Self {
        Self {
            text,
            line_num,
            note_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteType {
    Source,
    Author,
    Note,
    Journal,
    None,
}

impl NoteType {
    pub fn from_string(s: &str) -> Self {
        match s {
            "author" => Self::Author,
            "source" => Self::Source,
            "note" => Self::Note,
            "journal" => Self::Journal,
            _ => Self::None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Author => "author",
            Self::Note => "note",
            Self::Journal => "journal",
            Self::None => "none",
        }
    }
}

impl Display for NoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Ref {
    pub(crate) from: Option<RefNode>,
    pub(crate) to: RefNode,
    pub(crate) loc: RefLoc,
    // Kept for UI callers that want to show `[[id|alias]]` titles.
    #[allow(dead_code)]
    pub(crate) alias: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Anchor {
    pub(crate) note_id: Option<String>,
    pub(crate) title: String,
    pub(crate) heading_link: String,
    pub(crate) loc: RefLoc,
    pub(crate) level: usize,
}

impl Anchor {
    pub fn new(
        note_id: Option<String>,
        title: String,
        heading_link: String,
        loc: RefLoc,
        level: usize,
    ) -> Self {
        Self {
            note_id,
            title,
            heading_link,
            loc,
            level,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RefNode {
    NoteAnchor(String),
    Tag(String),
    Local(usize),
}

#[derive(Clone, Copy, Debug)]
pub enum RefLoc {
    FrontMatter(usize, usize, usize),
    Body(usize, usize, usize),
}

impl Ref {
    pub(crate) fn new(
        from: Option<RefNode>,
        to: RefNode,
        loc: RefLoc,
        alias: Option<String>,
    ) -> Self {
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
            Some(RefNode::Local(x)) => x,
            _ => 999,
        }
    }
}

impl Note {
    /// Find the reference under a cursor position.
    ///
    /// `line` and `col` are zero-based, matching the values returned by
    /// `nvim_win_get_cursor`.
    pub fn get_ref(&self, line: usize, col: usize) -> Option<Ref> {
        self.tags
            .iter()
            .chain(&self.links)
            .chain(&self.source_parents)
            .chain(&self.author)
            .find(|r| {
                let (ref_line, start_col, end_col) = r.loc.get_loc();
                ref_line == line && start_col <= col && col <= end_col
            })
            .cloned()
    }

    pub fn add_file_data(mut self, path: PathBuf, hash: u64, modified: SystemTime) -> Self {
        self.path = Some(path);
        self.hash = Some(hash);
        self.modified = Some(modified);
        self
    }
}

impl RefLoc {
    pub const fn get_loc(self) -> (usize, usize, usize) {
        match self {
            Self::FrontMatter(x, y, z) | Self::Body(x, y, z) => (x, y, z),
        }
    }

    pub const fn is_frontmatter(self) -> bool {
        matches!(self, Self::FrontMatter(..))
    }
}
