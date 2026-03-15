use std::path::Path;
use std::time::SystemTime;
use std::{fs, io};
use crate::Result;

use crate::note;
pub(crate) mod body;
pub(crate) mod frontmatter;

type LinesIter = io::Lines<io::BufReader<fs::File>>;

pub fn parse_note(path: &Path, mut lines: LinesIter, hash: u64, modified: SystemTime) -> Result<note::Note> {

    if let Some(Ok(l)) = lines.next() && l.as_str() == "---" {
        let x = frontmatter::FrontMatterParsing::parse(path, lines)?;
        let (mut x, iter) = body::BodyParsing::init(x)?;
        x.parse_lines(iter);
        Ok(x.consume(hash, modified))
    } else {
        Err(crate::MyError::ParseError("No frontmatter".to_string()))
    }
}

