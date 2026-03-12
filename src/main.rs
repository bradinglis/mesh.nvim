mod fold_rest;
mod parse;
mod note;
mod output;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::{self};
use std::io::{self, BufRead, Lines};
use std::path::Path;
use std::sync::LazyLock;
use std::time::Instant;
use thiserror::Error;

use crate::fold_rest::FoldRest;
// use crate::note;

pub type Result<T> = std::result::Result<T, MyError>;
#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("A custom error occurred")]
    OtherError,
}

#[derive(Clone, Debug)]
enum NotesPath {
    File(note::Note),
    Dir(Vec<NotesPath>),
}



fn get_notes(test: Vec<NotesPath>) -> Vec<note::Note> {
    let mut res = vec![];
    for x in test {
        match x {
            NotesPath::File(note) => res.push(note),
            NotesPath::Dir(notes_paths) => res.append(&mut get_notes(notes_paths)),
        }
    }

    res
}

fn get_note_outputs(test: Vec<NotesPath>) -> Vec<NoteOutput> {
    let mut res = vec![];
    for x in test {
        match x {
            NotesPath::File(note) => res.push(note.to_output()),
            NotesPath::Dir(notes_paths) => res.append(&mut get_note_outputs(notes_paths)),
        }
    }

    res
}

fn get_refs(test: &[Note]) -> Vec<Ref> {
    let mut res = vec![];

    test.iter().for_each(|x| {
        res.extend(x.tags.clone());
        res.extend(x.links.clone());
    });

    res
}


fn get_outputs(test: &[Note]) -> Vec<RefOutput> {
    let mut res = vec![];

    test.iter().for_each(|x| {
        x.tags.iter().for_each(|y| res.push(y.clone().to_output_struct()));
        x.links.iter().for_each(|y| res.push(y.clone().to_output_struct()));
    });

    res
}

fn main() -> Result<()> {
    let cache = Path::new("../testing/zettel/cache.json");
    // // Read the current directory (".").
    // let s = Instant::now();
    // let main_dir = Path::new("../testing/zettel/");
    // let test = read_dir(main_dir)?;
    // let x = get_notes(test);
    // // let y = get_refs(&x);
    //
    // // let out = Output {
    // //     references: y,
    // //     notes: x,
    // // };
    //
    // // let s = Instant::now();
    // // // println!("{}", serde_json::to_string(&out)?);
    // // let normal = s.elapsed();
    //
    // // let s2 = Instant::now();
    // //
    // // let aa = s2.elapsed();
    // //
    // // let out = OutputToo {
    // //     references: y,
    // //     notes: x,
    // // };
    //
    // // println!("{}", serde_json::to_string(&out)?);
    //
    // //
    // std::fs::write(cache, serde_json::to_string(&x)?)?;
    // //
    // let write = s.elapsed();
    //
    let s = Instant::now();
    let x: Vec<Note> = serde_json::from_str(&std::fs::read_to_string(cache)?)?;

    let y = get_refs(&x);
    // let notes = x.iter().map(|y| y.clone().to_output()).collect();

    // let out = OutputToo {
    //     references: y,
    //     notes,
    // };

    // let backlinks: Vec<RefOutput> = y.iter().filter_map(|x| {
    //     match x.clone().to {
    //         RefNode::Note(y) if y == "s_chambers_capitalist-economics_bankers-interest" => Some(x.clone().to_output_struct()),
    //         _ => None,
    //     }
    // }).collect();

    let contents: Vec<ContentLine> = x.iter().flat_map(|x| x.clone().contents).collect();

    let read = s.elapsed();
    println!("{}", serde_json::to_string_pretty(&contents)?);
    println!("read: {:#?}", read);

    Ok(())
}


fn read_file(file: &Path) -> Result<Note> {
    if file.extension().and_then(|x| x.to_str()).unwrap_or("") != "md" {
        return Err(MyError::OtherError);
    }

    let start = Instant::now();
    let file = fs::File::open(file)?;
    let reader = io::BufReader::new(file);

    let ((parsed, frontmatter_line_num), rest): ((ParseState, usize), _) = reader
        .lines()
        .fold_rest((ParseState::Nothing, 0), |(state, line_num), x| {
            let line_num = line_num + 1;
            let x = x?;
            match state {
                ParseState::InMd(note) => Ok(FoldState::Break((ParseState::InMd(note), line_num))),
                _ => Ok(FoldState::Continue((
                    parse_frontmatter(state, x)?,
                    line_num,
                ))),
            }
        })?;

    let note = match parsed {
        ParseState::InMd(note) => Ok(note),
        _ => Err(MyError::OtherError),
    }?;

    let mut parser = BodyParsing::init(note, frontmatter_line_num);

    parser.parse_lines(rest);

    let note = parser.consume();

    let duration = start.elapsed();
    // println!("Time for parsing note: {:?}", duration);

    Ok(note)
}


// fn parse_body(
//     content: BodyParsing,
//     z: std::result::Result<String, io::Error>,
//     note_id: &str,
// ) -> BodyParsing {
//     let text = z.unwrap_or("".to_string());
//     let line_num = line_num + 1;
//
//     let start = Instant::now();
//     let (tags, links, n_refs, n_ref_list) = parse_from_line(&text, note_id, &line_num);
//     let duration = start.elapsed();
//     println!("Time to parse line: {:?}", duration);
//
//     let start = Instant::now();
//     let body_tags = app(body_tags, tags);
//     let body_links = app(body_links, links);
//     let refs = app(refs, n_refs);
//     let ref_list = app(ref_list, n_ref_list);
//     let contents = push(
//         contents,
//         ContentLine {
//             note: note_id.to_string(),
//             text,
//             line_num,
//         },
//     );
//     let duration = start.elapsed();
//     println!("Time to add to vectors: {:?}", duration);
//
//     (contents, body_tags, body_links, refs, ref_list, line_num)
// }
//
fn app<T>(mut first: Vec<T>, mut second: Vec<T>) -> Vec<T> {
    first.append(&mut second);
    first
}

// fn app_refs(mut first: Vec<Ref>, mut second: InterimNote) -> Vec<Ref> {
//     first.append(&mut second.tags);
//     first.append(&mut second.links);
//     first.append(&mut second.references);
//     first
// }

fn push<T>(mut first: Vec<T>, second: T) -> Vec<T> {
    first.push(second);
    first
}

// fn parse_from_line(line: &str, id: &str, lnum: &usize) -> (Vec<Ref>, Vec<Ref>, Vec<Ref>, Vec<Ref>) {
//     let tags = content_get_tags(line, id, lnum);
//     let links = content_get_links(line, id, lnum);
//     let refs_intext = content_get_reference_intext(line, id, lnum);
//     let refs_list = content_get_reference_list(line, id, lnum);
//     (tags, links, refs_intext, refs_list)
// }

fn read_dir(dir: &Path) -> Result<Vec<NotesPath>> {
    let dir_name = dir.file_name().and_then(|x| x.to_str()).unwrap_or("");
    if dir_name == ".git" || dir_name == "templates" {
        return Err(MyError::OtherError);
    }

    let something: Vec<NotesPath> = fs::read_dir(dir)?
        .map(|entry| {
            let entry = entry?;
            read_path(entry.path().as_path())
        })
        .filter_map(|x| x.ok())
        .collect();

    Ok(something)
}

fn read_path(path: &Path) -> Result<NotesPath> {
    match (path.is_dir(), path.is_file()) {
        (true, _) => Ok(NotesPath::Dir(read_dir(path)?)),
        (_, true) => Ok(NotesPath::File(read_file(path)?)),
        (_, _) => Err(MyError::OtherError),
    }
    // Ok(())
}
