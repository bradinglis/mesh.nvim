mod fold_rest;
mod parse;
mod note;
mod file;
mod db;

use std::path::Path;
use thiserror::Error;

use crate::db::Db;

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
    
    // Cache file should be derived
    let cache_file = Path::new("/home/inglisb/.cache/markdown_db.duckdb");

    // Notes dir should be argument (passed from lua)
    let notes_dir = Path::new("/home/inglisb/zettel");

    let db = Db::init(notes_dir, cache_file)?;

    let (removing, adding) = file::Paths::go(&db)?;
    db.update_notes(adding, removing)?;

    Ok(())
}


