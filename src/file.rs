use crate::db::Db;
use crate::note::Note;
use crate::{Result, parse};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Paths {
    new: Vec<PathBuf>,
    modified: Vec<PathBuf>,
    deleted: Vec<PathBuf>,
    undecided: Vec<PathBuf>,
    current_state: HashMap<String, u64>,
}

impl Paths {
    pub(crate) fn go(db: &Db) -> Result<(Vec<String>, Vec<Note>, Vec<String>)> {
        let mut paths = Self::default();
        paths.add_from_dir(&db.notes_path);
        paths.current_state = db.get_hashes()?;
        Ok(paths.process())
    }

    fn add_from_dir(&mut self, dir: &Path) {
        let dir_name = dir.file_name().and_then(|x| x.to_str()).unwrap_or("");
        if dir_name == ".git" || dir_name == "templates" {
            return;
        }

        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.add_from_dir(&path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                self.undecided.push(path);
            }
        }
    }

    fn process(&mut self) -> (Vec<String>, Vec<Note>, Vec<String>) {
        while let Some(path) = self.undecided.pop() {
            let key = path.to_str().unwrap_or("");
            match self.current_state.remove(key) {
                Some(previous_hash)
                    if fs::File::open(&path)
                        .and_then(|file| rapidhash::v3::rapidhash_v3_file(&file))
                        .is_ok_and(|hash| hash != previous_hash) =>
                {
                    self.modified.push(path);
                }
                Some(_) => {}
                None => self.new.push(path),
            }
        }

        self.deleted = self.current_state.keys().map(PathBuf::from).collect();

        let mut message = vec!["Updated Index: ".to_string()];
        message.extend(
            self.modified
                .iter()
                .map(|path| format!("Modified: {}", path.display())),
        );
        message.extend(
            self.new
                .iter()
                .map(|path| format!("New: {}", path.display())),
        );
        message.extend(
            self.deleted
                .iter()
                .map(|path| format!("Deleted: {}", path.display())),
        );

        let to_delete = self
            .modified
            .iter()
            .chain(&self.deleted)
            .filter_map(|path| path.to_str())
            .map(str::to_owned)
            .collect();
        let to_create = self
            .modified
            .iter()
            .chain(&self.new)
            .filter_map(|path| process_file(path).ok())
            .collect();

        (to_delete, to_create, message)
    }
}

fn process_file(path: &Path) -> Result<Note> {
    let mut file = fs::File::open(path)?;
    let hash = rapidhash::v3::rapidhash_v3_file(&file)?;
    let modified = file.metadata()?.modified()?;

    // `rapidhash_v3_file` consumed the file, so rewind it rather than opening
    // the same path a second time.
    file.seek(SeekFrom::Start(0))?;
    let reader = io::BufReader::new(file);

    Ok(
        parse::parse_note(parse::LinesIter::File(reader.lines()))?.add_file_data(
            path.to_path_buf(),
            hash,
            modified,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::fs;

    #[test]
    fn scans_markdown_and_skips_special_dirs() {
        let base = std::env::temp_dir().join(format!(
            "mesh-file-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let notes = base.join("notes");
        fs::create_dir_all(notes.join(".git")).unwrap();
        fs::create_dir_all(notes.join("templates")).unwrap();
        fs::create_dir_all(notes.join("sub")).unwrap();
        fs::write(notes.join(".git").join("skip.md"), "x").unwrap();
        fs::write(notes.join("templates").join("skip.md"), "x").unwrap();
        fs::write(notes.join("root.md"), "x").unwrap();
        fs::write(notes.join("sub").join("nested.md"), "x").unwrap();
        fs::write(notes.join("ignore.txt"), "x").unwrap();

        let db_path = base.join("test.duckdb");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let db = Db::init(&notes, &db_path).unwrap();
        let (to_delete, to_create, _msg) = Paths::go(&db).unwrap();
        assert!(to_delete.is_empty());
        assert_eq!(to_create.len(), 2);
    }
}
