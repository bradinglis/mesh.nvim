//! Storage for chunk embeddings: schema sync, stale detection, obsolete-key
//! cleanup, and batched writes. Used by the semantic search pipeline.

use std::collections::{HashMap, HashSet};

use duckdb::{Row, params};

use crate::Result;
use crate::chunking::{AnchorSource, ChunkJob, ChunkKey, ChunkScope, NoteSource};
use crate::db::Db;

pub const EMBEDDING_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Default)]
pub struct EmbeddingPlan {
    pub to_embed: Vec<ChunkJob>,
    pub obsolete: Vec<ChunkKey>,
}

fn scope_from_str(scope: &str) -> ChunkScope {
    if scope == "note" {
        ChunkScope::Note
    } else {
        ChunkScope::Anchor
    }
}

fn read_key(row: &Row<'_>) -> duckdb::Result<ChunkKey> {
    Ok(ChunkKey {
        note_id: row.get(0)?,
        scope: scope_from_str(&row.get::<_, String>(1)?),
        anchor_id: row.get(2)?,
        part: row.get::<_, i32>(3)? as u32,
    })
}

impl Db {
    pub(crate) fn ensure_embedding_schema(&self) -> Result<()> {
        self.connection
            .execute_batch(include_str!("../sql/embeddings.sql"))?;
        Ok(())
    }

    /// Records the active model. If a different model or schema version was
    /// stored, all vectors are dropped. Returns whether that happened.
    pub fn sync_embedding_model(&self, model: &str, dim: usize) -> Result<bool> {
        let current: Option<(String, i64, i64)> = match self.connection.query_row(
            "select model, dim, schema_version from embedding_state",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ) {
            Ok(state) => Some(state),
            Err(duckdb::Error::QueryReturnedNoRows) => None,
            Err(error) => return Err(error.into()),
        };

        let wanted = (model.to_string(), dim as i64, EMBEDDING_SCHEMA_VERSION);
        if current.as_ref() == Some(&wanted) {
            return Ok(false);
        }

        self.connection
            .execute_batch("delete from embeddings; delete from embedding_state;")?;
        self.connection.execute(
            "insert into embedding_state values (?, ?, ?)",
            params![wanted.0, wanted.1, wanted.2],
        )?;
        Ok(current.is_some())
    }

    fn query_all<T>(
        &self,
        sql: &str,
        map: impl FnMut(&Row<'_>) -> duckdb::Result<T>,
    ) -> Result<Vec<T>> {
        let mut stmt = self.connection.prepare(sql)?;
        let rows = stmt.query_map([], map)?;
        Ok(rows.collect::<duckdb::Result<Vec<T>>>()?)
    }

    /// Loads the parsed-note view that [`crate::chunking::build_chunks`] needs.
    pub fn load_note_sources(&self) -> Result<Vec<NoteSource>> {
        let mut notes = self.query_all(
            "select note_id, note_title, note_type from notes order by note_id",
            |row| {
                Ok(NoteSource {
                    id: row.get(0)?,
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    note_type: row.get(2)?,
                    ..Default::default()
                })
            },
        )?;
        let index: HashMap<String, usize> = notes
            .iter()
            .enumerate()
            .map(|(i, note)| (note.id.clone(), i))
            .collect();

        for (note_id, tag) in self.query_all("select note_id, tag from tags", |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })? {
            if let Some(&i) = index.get(&note_id) {
                notes[i].tags.push(tag);
            }
        }
        for note in &mut notes {
            note.tags.sort();
            note.tags.dedup();
        }

        for (note_id, author) in self.query_all(
            "select r.note_id, coalesce(n.note_title, n.note_id)
             from refs r join notes n on n.note_id = r.to_anchor
             where r.kind = 'author'",
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )? {
            if let Some(&i) = index.get(&note_id) {
                notes[i].authors.push(author);
            }
        }

        for (note_id, anchor) in self.query_all(
            "select note_id, anchor_id, title, line from anchors where line is not null",
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    AnchorSource {
                        anchor_id: row.get(1)?,
                        heading: row.get(2)?,
                        line: row.get::<_, u32>(3)? as usize,
                    },
                ))
            },
        )? {
            if let Some(&i) = index.get(&note_id) {
                notes[i].anchors.push(anchor);
            }
        }

        for (note_id, line) in self.query_all(
            "select note_id, line, text from contents order by note_id, line",
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, u32>(1)? as usize, row.get::<_, String>(2)?),
                ))
            },
        )? {
            if let Some(&i) = index.get(&note_id) {
                notes[i].lines.push(line);
            }
        }

        Ok(notes)
    }

    /// Compares freshly built chunks with stored rows. A chunk needs embedding
    /// when it is missing or its text hash changed; stored keys that no longer
    /// exist in the vault are obsolete.
    pub fn plan_embeddings(&self, jobs: Vec<ChunkJob>) -> Result<EmbeddingPlan> {
        let stored: HashMap<ChunkKey, u64> = self
            .query_all(
                "select note_id, scope, anchor_id, part, text_hash from embeddings",
                |row| Ok((read_key(row)?, row.get::<_, u64>(4)?)),
            )?
            .into_iter()
            .collect();

        let wanted: HashSet<&ChunkKey> = jobs.iter().map(|job| &job.key).collect();
        let obsolete = stored
            .keys()
            .filter(|key| !wanted.contains(key))
            .cloned()
            .collect();
        let to_embed = jobs
            .into_iter()
            .filter(|job| stored.get(&job.key) != Some(&job.text_hash))
            .collect();

        Ok(EmbeddingPlan { to_embed, obsolete })
    }

    /// Replaces rows for `rows` and removes `obsolete` keys in one short
    /// transaction, so a crash leaves either the old or the new batch.
    pub fn write_embeddings(
        &self,
        obsolete: &[ChunkKey],
        rows: &[(ChunkJob, Vec<f32>)],
    ) -> Result<()> {
        self.connection.execute_batch("begin transaction")?;
        match self.apply_embeddings(obsolete, rows) {
            Ok(()) => {
                self.connection.execute_batch("commit")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("rollback");
                Err(error)
            }
        }
    }

    fn apply_embeddings(&self, obsolete: &[ChunkKey], rows: &[(ChunkJob, Vec<f32>)]) -> Result<()> {
        let mut delete = self.connection.prepare(
            "delete from embeddings
             where note_id = ? and scope = ? and anchor_id = ? and part = ?",
        )?;
        let mut insert = self
            .connection
            .prepare("insert into embeddings values (?, ?, ?, ?, ?, ?, ?, cast(? as FLOAT[]))")?;

        for key in obsolete {
            delete.execute(params![
                key.note_id,
                key.scope.as_str(),
                key.anchor_id,
                key.part
            ])?;
        }

        for (job, vector) in rows {
            let key = &job.key;
            delete.execute(params![
                key.note_id,
                key.scope.as_str(),
                key.anchor_id,
                key.part
            ])?;
            insert.execute(params![
                key.note_id,
                key.scope.as_str(),
                key.anchor_id,
                key.part,
                job.line.map(|line| line as u32),
                job.text_hash,
                job.text,
                vector_literal(vector),
            ])?;
        }
        Ok(())
    }
}

/// duckdb's parameter binding cannot send lists, so vectors are bound as text
/// and cast to `FLOAT[]` in SQL.
fn vector_literal(vector: &[f32]) -> String {
    let items: Vec<String> = vector.iter().map(f32::to_string).collect();
    format!("[{}]", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::build_chunks;
    use crate::embeddings::{Embedder, FakeEmbedder};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_vault() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "mesh-embed-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let notes = base.join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::create_dir_all(base.join("db")).unwrap();
        (notes, base.join("db").join("vault.duckdb"))
    }

    fn write_note(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(format!("{name}.md")), body).unwrap();
    }

    fn alpha(body: &str) -> String {
        format!(
            "---\nid: Alpha\naliases: Alpha\ntype: note\ntags: [topic]\n---\n# Alpha\n{body}\n## Details\nMore words here.\n"
        )
    }

    fn beta() -> String {
        "---\nid: Beta\naliases: Beta\ntype: source\ntags: []\n---\n# Beta\nSome beta text.\n"
            .to_string()
    }

    /// Returns (rows written, obsolete keys removed).
    fn sync(db: &Db, embedder: &dyn Embedder) -> (usize, usize) {
        db.sync_embedding_model(embedder.model_id(), embedder.dim())
            .unwrap();
        let jobs = build_chunks(&db.load_note_sources().unwrap());
        let EmbeddingPlan { to_embed, obsolete } = db.plan_embeddings(jobs).unwrap();
        let texts: Vec<String> = to_embed.iter().map(|job| job.text.clone()).collect();
        let vectors = embedder.embed(&texts).unwrap();
        let rows: Vec<(ChunkJob, Vec<f32>)> = to_embed.into_iter().zip(vectors).collect();
        db.write_embeddings(&obsolete, &rows).unwrap();
        (rows.len(), obsolete.len())
    }

    fn count(db: &Db, sql: &str) -> i64 {
        db.connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn embeddings_are_incremental_and_clean_up_removed_notes() {
        let (notes, db_path) = temp_vault();
        write_note(&notes, "alpha", &alpha("First body."));
        write_note(&notes, "beta", &beta());
        let db = Db::init(&notes, &db_path).unwrap();
        db.process_files().unwrap();
        let embedder = FakeEmbedder::standard();

        let (written, obsolete) = sync(&db, &embedder);
        assert!(written > 0);
        assert_eq!(obsolete, 0);
        let total = count(&db, "select count(*) from embeddings");
        assert_eq!(total as usize, written);

        let (written, obsolete) = sync(&db, &embedder);
        assert_eq!(
            (written, obsolete),
            (0, 0),
            "unchanged vault embeds nothing"
        );

        write_note(&notes, "alpha", &alpha("Changed body text."));
        db.process_files().unwrap();
        let (written, obsolete) = sync(&db, &embedder);
        assert!(
            written > 0 && (written as i64) < total,
            "only alpha is re-embedded"
        );
        assert_eq!(obsolete, 0);

        fs::remove_file(notes.join("beta.md")).unwrap();
        db.process_files().unwrap();
        let (_, obsolete) = sync(&db, &embedder);
        assert!(obsolete > 0);
        assert_eq!(
            count(
                &db,
                "select count(*) from embeddings where note_id = 'Beta'"
            ),
            0
        );
    }

    #[test]
    fn model_change_clears_stored_vectors() {
        let (notes, db_path) = temp_vault();
        write_note(&notes, "alpha", &alpha("Body."));
        let db = Db::init(&notes, &db_path).unwrap();
        db.process_files().unwrap();
        sync(&db, &FakeEmbedder::standard());
        assert!(count(&db, "select count(*) from embeddings") > 0);

        let other = FakeEmbedder {
            model: "other",
            dim: 8,
        };
        assert!(
            db.sync_embedding_model(other.model_id(), other.dim())
                .unwrap()
        );
        assert_eq!(count(&db, "select count(*) from embeddings"), 0);
    }

    #[test]
    fn existing_database_gets_embedding_tables_on_open() {
        let (notes, db_path) = temp_vault();
        write_note(&notes, "alpha", &alpha("Body."));
        {
            let db = Db::init(&notes, &db_path).unwrap();
            db.connection
                .execute_batch("drop table embeddings; drop table embedding_state;")
                .unwrap();
        }
        let db = Db::init(&notes, &db_path).unwrap();
        assert_eq!(count(&db, "select count(*) from embeddings"), 0);
        assert_eq!(count(&db, "select count(*) from embedding_state"), 0);
    }
}
