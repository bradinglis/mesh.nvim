//! Lua-free core API.
//!
//! Both the Neovim module (`src/lib.rs`) and the `mesh` CLI (`src/main.rs`)
//! call these functions, so their behaviour cannot drift. Functions take plain
//! Rust values and return [`DbValue`] (data) or a report string (writes); the
//! Lua bindings and the CLI are only thin adapters on top.

use std::path::Path;

use crate::db::{Db, DbValue};
use crate::semantic_search::{self, Query, SearchOpts};
use crate::{Result, ToNoteId};

/// Index the vault: scan for added, changed, and removed notes and update the
/// database. Opens the database read-write (creating it if needed) and returns
/// the human-readable log.
pub fn update(notes_dir: &str, db_file: &str) -> Result<String> {
    let db = Db::init(Path::new(notes_dir), Path::new(db_file))?;
    db.process_files()
}

pub fn notes(db: &Db) -> Result<DbValue> {
    db.get_notes()
}

pub fn lines(db: &Db) -> Result<DbValue> {
    db.get_lines()
}

pub fn thin_notes(db: &Db) -> Result<DbValue> {
    db.get_thin_notes()
}

pub fn type_notes(db: &Db, note_type: &str) -> Result<DbValue> {
    db.get_type_notes(note_type.to_string())
}

pub fn tags(db: &Db) -> Result<DbValue> {
    db.get_tag_list()
}

pub fn tag_notes(db: &Db, tag: &str) -> Result<DbValue> {
    db.get_tag_notes(tag.to_string())
}

pub fn note_anchors(db: &Db, id: &str) -> Result<DbValue> {
    db.get_note_anchors(id.to_string())
}

pub fn backlinks(db: &Db, id: &str) -> Result<DbValue> {
    db.get_backlinks(id.to_string())
}

/// Turn a title into its stable note id. Pure: no database access.
pub fn id_gen(title: &str) -> String {
    title.to_note_id()
}

pub fn search_prompt(
    db: &Db,
    db_file: &str,
    prompt: &str,
    context_note: Option<String>,
    opts: &SearchOpts,
) -> Result<DbValue> {
    let embedder = query_embedder(db_file)?;
    let query = Query::Prompt {
        text: prompt.to_string(),
        context_note,
    };
    run_search(db, embedder.as_ref(), &query, opts)
}

pub fn search_note(db: &Db, db_file: &str, note_id: &str, opts: &SearchOpts) -> Result<DbValue> {
    let embedder = query_embedder(db_file)?;
    run_search(
        db,
        embedder.as_ref(),
        &Query::Note(note_id.to_string()),
        opts,
    )
}

fn run_search(
    db: &Db,
    embedder: &dyn crate::embeddings::Embedder,
    query: &Query,
    opts: &SearchOpts,
) -> Result<DbValue> {
    let hits = semantic_search::search(db, embedder, query, opts)?;
    Ok(DbValue::List(
        hits.iter().map(semantic_search::Hit::to_db_value).collect(),
    ))
}

#[cfg(feature = "semantic")]
fn query_embedder(db_file: &str) -> Result<Box<dyn crate::embeddings::Embedder>> {
    Ok(Box::new(crate::embeddings::OnnxEmbedder::load(Path::new(
        db_file,
    ))?))
}

#[cfg(not(feature = "semantic"))]
fn query_embedder(_db_file: &str) -> Result<Box<dyn crate::embeddings::Embedder>> {
    Err(crate::MyError::String(
        "built without the semantic feature".to_string(),
    ))
}

#[cfg(feature = "semantic")]
pub fn embed_refresh(
    notes_dir: &str,
    db_file: &str,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<String> {
    let embedder = crate::embeddings::OnnxEmbedder::load(Path::new(db_file))?;
    let report = crate::embed_index::update_embeddings(
        &embedder,
        Path::new(notes_dir),
        Path::new(db_file),
        progress,
    )?;
    Ok(format!(
        "Embeddings: {} written, {} removed{}",
        report.embedded,
        report.removed,
        if report.cleared {
            " (model changed, index rebuilt)"
        } else {
            ""
        }
    ))
}

#[cfg(not(feature = "semantic"))]
pub fn embed_refresh(
    _notes_dir: &str,
    _db_file: &str,
    _progress: &mut dyn FnMut(usize, usize),
) -> Result<String> {
    Err(crate::MyError::String(
        "built without the semantic feature".to_string(),
    ))
}

#[cfg(feature = "semantic")]
pub fn warm_model(db_file: &str) -> Result<String> {
    crate::embeddings::OnnxEmbedder::load(Path::new(db_file))?;
    Ok("embedding model ready".to_string())
}

#[cfg(not(feature = "semantic"))]
pub fn warm_model(_db_file: &str) -> Result<String> {
    Err(crate::MyError::String(
        "built without the semantic feature".to_string(),
    ))
}
