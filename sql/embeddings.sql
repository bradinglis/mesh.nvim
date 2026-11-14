CREATE TABLE IF NOT EXISTS embedding_state (
  model TEXT NOT NULL,
  dim INTEGER NOT NULL,
  schema_version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS embeddings (
  note_id TEXT NOT NULL,
  scope TEXT NOT NULL,
  anchor_id TEXT NOT NULL,
  part INTEGER NOT NULL,
  line UINTEGER NULL,
  text_hash UBIGINT NOT NULL,
  text TEXT NOT NULL,
  vector FLOAT[] NOT NULL,
  PRIMARY KEY (note_id, scope, anchor_id, part)
);
