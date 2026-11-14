//! Embedding refresh: plan with a short database handle, embed without one, then
//! write each batch in its own short transaction. Progress is durable, so an
//! interrupted run resumes on the next refresh.

use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use crate::chunking::{ChunkJob, ChunkKey, build_chunks};
use crate::db::Db;
use crate::embeddings::Embedder;
use crate::{MyError, Result};

pub const BATCH_SIZE: usize = 32;
const WRITE_ATTEMPTS: u32 = 5;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub cleared: bool,
    pub embedded: usize,
    pub removed: usize,
}

pub fn update_embeddings(
    embedder: &dyn Embedder,
    notes_dir: &Path,
    db_file: &Path,
) -> Result<IndexReport> {
    let (cleared, obsolete, to_embed) = {
        let db = Db::init(notes_dir, db_file)?;
        let cleared = db.sync_embedding_model(embedder.model_id(), embedder.dim())?;
        let plan = db.plan_embeddings(build_chunks(&db.load_note_sources()?))?;
        (cleared, plan.obsolete, plan.to_embed)
    };

    let mut report = IndexReport {
        cleared,
        embedded: 0,
        removed: obsolete.len(),
    };
    let mut pending_removals = obsolete;
    let mut jobs = to_embed;

    while !jobs.is_empty() {
        let batch: Vec<ChunkJob> = jobs.drain(..jobs.len().min(BATCH_SIZE)).collect();
        let texts: Vec<String> = batch.iter().map(|job| job.text.clone()).collect();
        let vectors = embedder.embed(&texts)?;
        if vectors.len() != batch.len() {
            return Err(MyError::String(format!(
                "embedder returned {} vectors for {} chunks",
                vectors.len(),
                batch.len()
            )));
        }
        let rows: Vec<(ChunkJob, Vec<f32>)> = batch.into_iter().zip(vectors).collect();
        write_with_retry(
            notes_dir,
            db_file,
            &std::mem::take(&mut pending_removals),
            &rows,
        )?;
        report.embedded += rows.len();
    }

    if !pending_removals.is_empty() {
        write_with_retry(notes_dir, db_file, &pending_removals, &[])?;
    }

    Ok(report)
}

/// The file lock is per-process and writers can conflict under MVCC, so a
/// failed write is retried a few times with a growing pause.
fn write_with_retry(
    notes_dir: &Path,
    db_file: &Path,
    obsolete: &[ChunkKey],
    rows: &[(ChunkJob, Vec<f32>)],
) -> Result<()> {
    let mut attempt = 1;
    loop {
        let result =
            Db::init(notes_dir, db_file).and_then(|db| db.write_embeddings(obsolete, rows));
        match result {
            Ok(()) => return Ok(()),
            Err(MyError::DuckDb(_)) if attempt < WRITE_ATTEMPTS => {
                sleep(Duration::from_millis(50 * u64::from(attempt)));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::FakeEmbedder;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_vault() -> (std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "mesh-index-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let notes = base.join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::create_dir_all(base.join("db")).unwrap();
        (notes, base.join("db").join("vault.duckdb"))
    }

    #[test]
    fn refresh_is_incremental_and_removes_deleted_notes() {
        let (notes, db_file) = temp_vault();
        let body = |text: &str| {
            format!("---\nid: Alpha\naliases: Alpha\ntype: note\ntags: []\n---\n# Alpha\n{text}\n")
        };
        fs::write(notes.join("alpha.md"), body("first")).unwrap();
        fs::write(
            notes.join("beta.md"),
            "---\nid: Beta\naliases: Beta\ntype: note\ntags: []\n---\n# Beta\nbeta text\n",
        )
        .unwrap();
        Db::init(&notes, &db_file).unwrap().process_files().unwrap();

        let embedder = FakeEmbedder::standard();
        let first = update_embeddings(&embedder, &notes, &db_file).unwrap();
        assert!(!first.cleared && first.embedded > 0);

        let second = update_embeddings(&embedder, &notes, &db_file).unwrap();
        assert_eq!(
            second,
            IndexReport {
                cleared: false,
                embedded: 0,
                removed: 0
            }
        );

        fs::remove_file(notes.join("beta.md")).unwrap();
        Db::init(&notes, &db_file).unwrap().process_files().unwrap();
        let third = update_embeddings(&embedder, &notes, &db_file).unwrap();
        assert!(third.removed > 0);
        assert_eq!(third.embedded, 0);
    }

    #[test]
    fn large_sections_are_embedded_in_batches() {
        let (notes, db_file) = temp_vault();
        let mut text =
            String::from("---\nid: Big\naliases: Big\ntype: note\ntags: []\n---\n# Big\n");
        for i in 0..200 {
            text.push_str(&format!(
                "## Heading {i}\n\nBody paragraph number {i} with some words.\n\n"
            ));
        }
        fs::write(notes.join("big.md"), text).unwrap();
        Db::init(&notes, &db_file).unwrap().process_files().unwrap();

        let report = update_embeddings(&FakeEmbedder::standard(), &notes, &db_file).unwrap();
        assert!(
            report.embedded > BATCH_SIZE,
            "expected multiple batches, got {}",
            report.embedded
        );
        let db = Db::init(&notes, &db_file).unwrap();
        let stored: i64 = db
            .connection
            .query_row("select count(*) from embeddings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stored as usize, report.embedded);
    }
}
