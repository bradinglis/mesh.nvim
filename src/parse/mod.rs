use crate::Result;
use crate::note;
use std::{fs, io, iter::Peekable};

pub(crate) mod body;
pub(crate) mod frontmatter;

// type LinesIter = io::Lines<io::BufReader<fs::File>>;
pub type Lines = Peekable<LinesIter>;

pub enum LinesIter {
    File(io::Lines<io::BufReader<fs::File>>),
    List(std::vec::IntoIter<String>),
}

impl Iterator for LinesIter {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            LinesIter::File(lines) => lines.next().map(|x| x.map_err(|x| x.into())),
            LinesIter::List(items) => items.next().map(Ok),
        }
    }
}

pub(crate) fn parse_note(lines: LinesIter) -> Result<note::Note> {
    let lines: Lines = lines.peekable();
    let (x, y) = frontmatter::FrontMatter::parse(lines)?;
    Ok(body::BodyParsing::init(x)?
        .parse_lines(y)
        .consume())
}
