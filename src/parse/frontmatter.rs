use std::path::PathBuf;
use std::str::SplitWhitespace;

use crate::MyError;
use crate::Result;
use crate::fold_rest::{FoldRest, IterState};
use crate::note::{ContentLine, Note, NoteType, Ref, RefLoc, RefNode};
use crate::parse::LinesIter;
use crate::parse::body::InterimNote;

// #[derive(Debug)]
pub(crate) struct FrontMatterParsing {
    id: Option<String>,
    aliases: Option<String>,
    note_type: Option<String>,
    tags: Vec<Ref>,
    references: Vec<Ref>,
    source_parents: Vec<String>,
    author: Vec<String>,
    rest: Option<LinesIter>,
    path: PathBuf,
}

impl FrontMatterParsing {
    pub fn parse(path: PathBuf, iter: LinesIter) -> Result<InterimNote> {
        let mut new = Self::new(&path);

        let line_num: usize = 0;
        let rest = iter.for_each_rest(|x| {
            let line = x?;
            line_num += 0;
            // let frontmatter_boundary = line == "---";
            match line.as_str() {
                "---" => Ok(IterState::Break),
                _ => {
                    new.add_line_to_frontmatter(line);
                    Ok(IterState::Continue)
                }
            }
        })?;

        let id = new.id.ok_or(MyError::OtherError)?;
        let note_type = new.note_type.ok_or(MyError::OtherError)?;
        Ok(InterimNote::new(
            id,
            new.aliases,
            note_type_from_string(note_type)?,
            new.tags,
            new.references,
            new.source_parents,
            new.author,
            line_num,
            Some(rest),
            path,
        ))
        // match (state, frontmatter_boundary) {
        //     (ParseState::Nothing, true) => Ok(init_frontmatter_parse()),
        //     (ParseState::InYaml(front_matter_build), false) => Ok(ParseState::InYaml(
        //         add_line_to_frontmatter(line, front_matter_build),
        //     )),
        //     (ParseState::InYaml(front_matter_build), true) => format_frontmatter(front_matter_build),
        //     _ => Err(MyError::OtherError),
        // }
    }

    fn add_line_to_frontmatter(&mut self, line: String) -> Result<()> {
        Ok(match line.split_once(": ") {
            Some(("id", suff)) => self.id = Some(suff.to_string()),
            Some(("aliases", suff)) => self.aliases = Some(suff.to_string()),
            Some(("type", suff)) => self.note_type = Some(suff.to_string()),
            Some(("source-parents", suff)) => self.add_source_parents(suff),
            Some(("author", suff)) => self.add_authors(suff),
            Some(("references", suff)) => self.add_references(suff),
            Some(("tags", suff)) => self.add_tags(suff),
            _ => Err(MyError::OtherError)?,
        })
    }

    fn add_source_parents(&mut self, text: &str) -> () {
        match Self::get_text_list(text).map(|iter| iter.map(|parent| parent.to_string()).collect())
        {
            Some(parents) => self.source_parents = parents,
            None => (),
        }
    }

    fn add_authors(&mut self, text: &str) -> () {
        match Self::get_text_list(text).map(|iter| iter.map(|author| author.to_string()).collect())
        {
            Some(authors) => self.author = authors,
            None => (),
        }
    }

    fn add_tags(&mut self, text: &str) -> () {
        match Self::get_text_list(text).map(|iter| {
            iter.map(|tag| {
                Ref::new(
                    RefNode::Note(self.id.clone().unwrap_or("".to_string())),
                    RefNode::Tag(tag.to_string()),
                    RefLoc::FrontMatter,
                    None,
                )
            })
            .collect()
        }) {
            Some(tags) => self.tags = tags,
            _ => (),
        }
    }

    fn add_references(&mut self, text: &str) -> () {
        match Self::get_text_list(text).map(|iter| {
            iter.map(|reference| {
                Ref::new(
                    RefNode::Note(self.id.clone().unwrap_or("".to_string())),
                    RefNode::Note(reference.to_string()),
                    RefLoc::FrontMatter,
                    None,
                )
            })
            .collect()
        }) {
            Some(references) => self.references = references,
            _ => (),
        }
    }

    fn get_text_list(text: &str) -> Option<SplitWhitespace<'_>> {
        match text {
            "[]" => None,
            y => Some(y.split_whitespace()),
        }
    }

    fn new(path: &PathBuf) -> Self {
        Self {
            id: None,
            aliases: None,
            note_type: None,
            tags: vec![],
            references: vec![],
            source_parents: vec![],
            author: vec![],
            path: path.clone(),
            rest: None,
        }
    }
}

// fn parse_frontmatter(state: ParseState, line: String) -> Result<ParseState> {
//     let frontmatter_boundary = line == "---";
//
//     match (state, frontmatter_boundary) {
//         (ParseState::Nothing, true) => Ok(init_frontmatter_parse()),
//         (ParseState::InYaml(front_matter_build), false) => Ok(ParseState::InYaml(
//             add_line_to_frontmatter(line, front_matter_build),
//         )),
//         (ParseState::InYaml(front_matter_build), true) => format_frontmatter(front_matter_build),
//         _ => Err(MyError::OtherError),
//     }
// }

// fn init_frontmatter_parse() -> ParseState {
//     ParseState::InYaml(FrontMatterParsing {
//         id: None,
//         aliases: None,
//         note_type: None,
//         tags: vec![],
//         references: vec![],
//         source_parents: vec![],
//         author: vec![],
//     })
// }

//
// fn format_frontmatter(front_matter_build: FrontMatterParsing) -> Result<ParseState> {
//     let note_type = front_matter_build.note_type.ok_or(MyError::OtherError)?;
//     let id = front_matter_build.id.ok_or(MyError::OtherError)?;
//     InterimNote {
//         id: id.clone(),
//         aliases: front_matter_build.aliases,
//         note_type: note_type_from_string(note_type)?,
//         tags: front_matter_build
//             .tags
//             .unwrap_or(vec![])
//             .iter()
//             .map(|x| Ref {
//                 from: RefNode::Note(id.clone()),
//                 to: RefNode::Tag(x.to_string()),
//                 loc: RefLoc::FrontMatter,
//                 alias: None,
//             })
//             .collect(),
//         references: front_matter_build
//             .references
//             .unwrap_or(vec![])
//             .iter()
//             .map(|x| Ref {
//                 from: RefNode::Note(id.clone()),
//                 to: RefNode::Note(x.to_string()),
//                 loc: RefLoc::FrontMatter,
//                 alias: None,
//             })
//             .collect(),
//         source_parents: front_matter_build.source_parents.unwrap_or(vec![]),
//         author: front_matter_build.author.unwrap_or(vec![]),
//     }))
// }

fn note_type_from_string(s: String) -> Result<NoteType> {
    match s.as_str() {
        "author" => Ok(NoteType::Author),
        "source" => Ok(NoteType::Source),
        "note" => Ok(NoteType::Note),
        _ => Err(MyError::OtherError),
    }
}
