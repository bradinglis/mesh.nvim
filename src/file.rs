use serde::{Deserialize, Serialize};

use crate::note::{ContentLine, Note, NoteType, Ref, RefLoc, RefNode};
use crate::output;
use crate::{MyError, MyResult, Result};
use duckdb::Connection;
use std::collections::{HashMap, hash_map};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use crate::note;
use crate::parse;

#[derive(Clone, Debug, Serialize, Deserialize)]
enum NotesPath {
    File(PathBuf),
    Dir(Vec<NotesPath>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paths {
    new: Vec<PathBuf>,
    modified: Vec<PathBuf>,
    deleted: Vec<PathBuf>,
    undecided: Vec<PathBuf>,
    current_state: HashMap<String, u64>,
}

impl Paths {
    pub fn go() -> Result<(Vec<String>, Vec<Note>)> {
        let mut x = Self::init();
        x.add_from_dir(PathBuf::from("/home/brad/zettel/"));
        let _ = x.get_db_state().i();
        Ok(x.process())
    }

    // fn add_new(&mut self, path: &Path) {
    //     self.new.push(path.to_path_buf());
    // }
    //
    // fn add_modified(&mut self, path: &Path) {
    //     self.modified.push(path.to_path_buf());
    // }
    //
    // fn add_deleted(&mut self, path: &Path) {
    //     self.deleted.push(path.to_path_buf());
    // }

    fn init() -> Self {
        Self {
            new: vec![],
            modified: vec![],
            deleted: vec![],
            undecided: vec![],
            current_state: HashMap::new(),
        }
    }

    fn add_from_dir(&mut self, dir: PathBuf) {
        let dir_name = dir.file_name().and_then(|x| x.to_str()).unwrap_or("");
        if dir_name == ".git" || dir_name == "templates" {
            return;
        }
        if let Ok(d) = fs::read_dir(dir) {
            d.for_each(|entry| {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    match (path.is_dir(), path.is_file()) {
                        (true, _) => self.add_from_dir(path),
                        (_, true) => {
                            if path.extension().and_then(|x| x.to_str()).unwrap_or("") == "md" {
                                self.undecided.push(path)
                            }
                        }
                        _ => (),
                    };
                }
            });
        }
    }

    fn process(&mut self) -> (Vec<String>, Vec<Note>) {
        while let Some(path) = self.undecided.pop() {
            if let Some(x) = self.current_state.remove(path.to_str().unwrap_or("")) {
                if let Ok(file) = fs::File::open(&path)
                    && let Ok(hash) = rapidhash::v3::rapidhash_v3_file(&file)
                    && hash != x
                {
                    println!("{:?} modified", path.to_str());
                    self.modified.push(path);
                }
            } else {
                println!("{:?} new", path.to_str());
                self.new.push(path);
            }
        }

        let remaining = self.current_state.keys();
        for x in remaining {
            let x = PathBuf::from(x);
            self.deleted.push(x);
        }

        let to_delete: Vec<String> = [self.modified.clone(), self.deleted.clone()]
            .concat()
            .iter()
            .map(|x| x.to_str().unwrap_or("").to_string())
            .collect();
        let to_create: Vec<Note> = [self.modified.clone(), self.new.clone()]
            .concat()
            .iter()
            .filter_map(|x| process_file(x).ok())
            .collect();

        (to_delete, to_create)
    }

    fn get_db_state(&mut self) -> Result<()> {
        let conn = Connection::open("/home/brad/.cache/markdown_db.duckdb")?;
        let mut stmt = conn.prepare("select path, hash from notes")?;


        let mut x = stmt.query([])?;

        while let Some(y) = x.next()? {
            self.current_state.insert(y.get(0)?, y.get(1)?);
        }
        // let _x = stmt.query_map([], |x| {
        //     println!("yo");
        //     let y: String = x.get(0)?;
        //     self.current_state.insert(x.get(0)?, x.get(1)?);
        //     Ok(())
        // })?;

        println!("what!");
        // println!("{:#?}", self);

        Ok(())
    }
}

fn get_paths(test: Vec<NotesPath>) -> Result<Vec<PathBuf>> {
    fn from_vec(test: Vec<NotesPath>, res: &mut Vec<PathBuf>) -> Result<()> {
        for x in test {
            match x {
                NotesPath::File(note) => res.push(note),
                NotesPath::Dir(notes_paths) => from_vec(notes_paths, res)?,
            }
        }
        Ok(())
    }

    let mut res: Vec<PathBuf> = vec![];
    for x in test {
        match x {
            NotesPath::File(note) => res.push(note),
            NotesPath::Dir(notes_paths) => from_vec(notes_paths, &mut res)?,
        }
    }
    Ok(res)
}

// pub fn read_root(root: &Path) -> Result<Vec<note::Note>> {
//     let x = Paths::init();
//
//     let something: Vec<&Path> = fs::read_dir(root)?
//         .map(|entry| {
//             let entry = entry?;
//             read_path(entry.path().as_path())
//         })
//         .filter_map(|x| x.ok())
//         .collect();
//
//     // match (path.is_dir(), path.is_file()) {
//     //     (true, _) => Ok(NotesPath::Dir(read_dir(path)?)),
//     //     (_, true) => Ok(NotesPath::File(read_file(path)?)),
//     //     (_, _) => Err(MyError::FileError("path is neither directory or file".to_string())),
//     // }
// }

fn process_file(path: &Path) -> Result<Note> {
    let file = fs::File::open(path)?;
    let modified = file.metadata()?.modified()?;
    let hash = rapidhash::v3::rapidhash_v3_file(&file)?;
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let note = parse::parse_note(path, reader.lines(), hash, modified).i()?;

    Ok(note)
}
