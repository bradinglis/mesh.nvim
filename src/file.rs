ubse serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::note::Note;
use crate::{MyResult, Result, parse};
use std::collections::{HashMap};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paths {
    new: Vec<PathBuf>,
    modified: Vec<PathBuf>,
    deleted: Vec<PathBuf>,
    undecided: Vec<PathBuf>,
    current_state: HashMap<String, u64>,
}

impl Paths {
    pub(crate) fn go(db: &Db) -> Result<(Vec<String>, Vec<Note>, Vec<String>)> {
        let mut x = Self::init();
        x.add_from_dir(&db.notes_path);
        x.get_db_state(db).i()?;
        Ok(x.process())
    }


    fn init() -> Self {
        Self {
            new: vec![],
            modified: vec![],
            deleted: vec![],
            undecided: vec![],
            current_state: HashMap::new(),
        }
    }

    fn add_from_dir(&mut self, dir: &PathBuf) {
        let dir_name = dir.file_name().and_then(|x| x.to_str()).unwrap_or("");
        if dir_name == ".git" || dir_name == "templates" {
            return;
        }
        if let Ok(d) = fs::read_dir(dir) {
            d.for_each(|entry| {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    match (path.is_dir(), path.is_file()) {
                        (true, _) => self.add_from_dir(&path),
                        (_, true) if path.extension().and_then(|x| x.to_str()).unwrap_or("") == "md" => self.undecided.push(path),
                        _ => (),
                    };
                }
            });
        }
    }

    fn process(&mut self) -> (Vec<String>, Vec<Note>, Vec<String>) {
        while let Some(path) = self.undecided.pop() {
            if let Some(x) = self.current_state.remove(path.to_str().unwrap_or("")) {
                if let Ok(file) = fs::File::open(&path)
                    && let Ok(hash) = rapidhash::v3::rapidhash_v3_file(&file)
                    && hash != x
                {
                    self.modified.push(path);
                }
            } else {
                self.new.push(path);
            }
        }

        let remaining = self.current_state.keys();
        for x in remaining {
            let x = PathBuf::from(x);
            self.deleted.push(x);
        }


        // Somehow pass this back to nvim...
        let mut message: Vec<String> = vec!["Updated Index: ".to_string()];
        for m in self.modified.clone() {
            message.push(format!("Modified: {}", m.to_string_lossy()))
        }

        for n in self.new.clone() {
            message.push(format!("New: {}", n.to_string_lossy()))
        }

        for d in self.deleted.clone() {
            message.push(format!("Deleted: {}", d.to_string_lossy()))
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

        (to_delete, to_create, message)
    }

    fn get_db_state(&mut self, db: &Db) -> Result<()> {

        self.current_state = db.get_hashes()?;

        Ok(())
    }
}

fn process_file(path: &Path) -> Result<Note> {
    let file = fs::File::open(path)?;
    let modified = file.metadata()?.modified()?;
    let hash = rapidhash::v3::rapidhash_v3_file(&file)?;
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let body_parse = parse::parse_note(parse::LinesIter::File(reader.lines()))?;
    let note = body_parse.consume(Some(path.to_path_buf()),Some(hash), Some(modified));

    Ok(note)
}
