use std::path::Path;
use std::time::SystemTime;
use std::{fs, io};
use crate::Result;

use crate::note;
pub(crate) mod body;
pub(crate) mod frontmatter;

// type LinesIter = io::Lines<io::BufReader<fs::File>>;
pub enum LinesIter {
    File(io::Lines<io::BufReader<fs::File>>),
    List(std::vec::IntoIter<String>)
}

impl LinesIter {
    pub fn next(&mut self) -> Option<Result<String>>{
        match self {
            LinesIter::File(lines) => lines.next().map(|x| x.map_err(|x| x.into())),
            LinesIter::List(items) => items.next().map(Ok),
        }
    }
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

pub(crate) fn parse_note(mut lines: LinesIter) -> Result<body::BodyParsing> {
    if let Some(Ok(l)) = lines.next() && l.as_str() == "---" {
        let x = frontmatter::FrontMatterParsing::parse(lines)?;
        let (mut x, iter) = body::BodyParsing::init(x)?;
        x.parse_lines(iter);
        Ok(x)
        // Ok(x.consume(hash, modified))
    } else {
        Err(crate::MyError::ParseError("No frontmatter".to_string()))
    }
}

