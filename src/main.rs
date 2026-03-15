mod fold_rest;
mod parse;
mod note;
mod output;
mod file;
mod db;
mod lib;

use std::collections::HashMap;
use std::env;
use std::io::{BufReader, BufWriter, stdout};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use thiserror::Error;
use duckdb::{params, Connection};
use rapidhash;

use crate::db::Db;
// use crate::file::NotesPath;
use crate::lib::db_get;

pub type Result<T> = std::result::Result<T, MyError>;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("File error: {0}")]
    FileError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("A custom error occurred")]
    OtherError,
}

pub trait MyResult {
    
    fn i(self) -> Self; 
}

impl<T> MyResult for Result<T> {

    fn i(self) -> Self {
        match self {
            Err(y) => {
                eprintln!("Error: {}", y);
                Err(y)
            },
            x => x,
        }

    }
}

fn main() -> Result<()> {
    // let path = Path::new("/home/brad/zettel/notes/abstract-labour.md");
    // // let file = fs::File::open(path);
    // let hash = rapidhash::v3::rapidhash_v3_file(file)?;
    //

    // let x = db_get()?;

    // println!("{:#?}", x);
    
    let cache_file = Path::new("/home/brad/.cache/markdown_db.duckdb");
    // let main_dir = Path::new("../zettel/");
    //
    // let s = Instant::now();
    // let mut args = env::args();
    //
    let conn = Connection::open(cache_file)?;
    //
    let (test, test2) = file::Paths::go()?;

    print!("{:#?}", test);
    print!("{:#?}", test2);

    let x = Db::from_notes(test2, test)?;

    // x.create_db(&conn)?;
    //
    x.commit_db(conn).i()?;
    //
    // // let _arg0 = args.next();
    // //
    // // let arg1 = args.next();
    // // let arg2 = args.next();
    // //
    // // let first = arg1.as_deref();
    // // let second = arg2.as_deref();
    // //
    // // match (first, second) {
    // //     (Some("index"), _) => index(cache_file, main_dir)?,
    // //     (Some("list"), Some("tags")) => list_tags(cache_file)?,
    // //     (Some("list"), Some("notes")) => list_notes(cache_file)?,
    // //     (Some("list"), Some("contents")) => index(cache_file, main_dir)?,
    // //     _ => print_help(),
    // // };
    //
    //
    //
    // // let x = get_notes(test);
    // // let y = get_refs(&x);
    // //
    // // // let out = Output {
    // // //     references: y,
    // // //     notes: x,
    // // // };
    // //
    // // // let s = Instant::now();
    // // // // println!("{}", serde_json::to_string(&out)?);
    // // // let normal = s.elapsed();
    // //
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
    // let s = Instant::now();
    // let x: Vec<Note> = serde_json::from_str(&std::fs::read_to_string(cache)?)?;
    //
    // let y = get_refs(&x);
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

    // let contents: Vec<ContentLine> = x.iter().flat_map(|x| x.clone().contents).collect();
    //
    // let read = s.elapsed();
    // println!("{}", serde_json::to_string_pretty(&contents)?);
    // println!("read: {:#?}", read);

    Ok(())
}

fn print_help() -> () {
    println!("What!!");
}

// fn index(cache_file: &Path, main_dir: &Path) -> Result<()> {
//     // let file = std::fs::File::create(cache_file)?;
//     let test = file::read_path(main_dir).i()?.get_note_outputs()?;
//
//     let x = Db::from_notes(test)?;
//
//     // let writer = BufWriter::new(file);
//     let writer = BufWriter::new(stdout());
//
//     serde_json::to_writer_pretty(writer, &x)?;
//     Ok(())
// }
//
// fn list_notes(cache_file: &Path) -> Result<()> {
//     let file = std::fs::File::open(cache_file)?;
//     let reader = BufReader::new(file);
//     let x: Vec<note::Note> = serde_json::from_reader(reader)?;
//
//
//     let writer = BufWriter::new(stdout());
//     serde_json::to_writer(writer, &x)?;
//
//     Ok(())
//
// }
//
// fn list_tags(cache_file: &Path) -> Result<()> {
//
//     let file = std::fs::File::open(cache_file)?;
//     let reader = BufReader::new(file);
//     let x: Vec<note::Note> = serde_json::from_reader(reader)?;
//
//     let tags = tags(x);
//
//     let writer = BufWriter::new(stdout());
//     serde_json::to_writer_pretty(writer, &tags)?;
//
//     Ok(())
//
// }
//
// fn tags(test: Vec<note::Note>) -> HashMap<String, Vec<note::Ref>> {
//     // let mut res = vec![];
//     // test.iter().for_each(|x| {
//     //     res.extend(x.tags.clone());
//     // });
//
//     let grouped: HashMap<String, Vec<note::Ref>> = 
//         test.into_iter().fold(HashMap::new(), |mut acc, val| {
//             val.tags.into_iter().for_each(|tag| {
//                 if let Ok(x) = tag.get_tag() {
//                     acc.entry(x).or_default().push(tag);
//                 }
//             });
//             acc
//         }
// );
//     //     res.into_iter().fold(HashMap::new(), |mut acc, val| {
//     //     if let Ok(x) = val.get_tag() {
//     //         acc.entry(x).or_default().push(val);
//     //     }
//     //     acc
//     // });
//
//     grouped
// }
//
// //
// //
// // fn get_outputs(test: &[Note]) -> Vec<RefOutput> {
// //     let mut res = vec![];
// //
// //     test.iter().for_each(|x| {
// //         x.tags.iter().for_each(|y| res.push(y.clone().to_output_struct()));
// //         x.links.iter().for_each(|y| res.push(y.clone().to_output_struct()));
// //     });
// //
// //     res
// // }
// //
//
// // fn get_notes(test: Vec<NotesPath>) -> Vec<note::Note> {
// //     let mut res = vec![];
// //     for x in test {
// //         match x {
// //             NotesPath::File(note) => res.push(note),
// //             NotesPath::Dir(notes_paths) => res.append(&mut get_notes(notes_paths)),
// //         }
// //     }
// //     res
// // }
