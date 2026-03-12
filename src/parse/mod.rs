use std::path::PathBuf;
use std::{fs, io};
use crate::MyError;
use crate::Result;

use crate::note;
pub(crate) mod body;
pub(crate) mod frontmatter;

type LinesIter = io::Lines<io::BufReader<fs::File>>;

// #[derive(Debug)]
enum ParseState {
    Nothing,
    InYaml(frontmatter::FrontMatterParsing),
    InMd(body::BodyParsing),
}

pub fn parse_note(path: PathBuf, lines: LinesIter) -> Result<note::Note> {
    let x = frontmatter::FrontMatterParsing::parse(path, lines)?;
    let (mut x, iter) = body::BodyParsing::init(x)?;
    x.parse_lines(iter);
    Ok(x.consume())

}

