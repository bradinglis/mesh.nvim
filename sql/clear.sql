DROP TABLE if exists tags;
DROP TABLE if exists parents;
DROP TABLE if exists refs;
DROP TABLE if exists authors;
DROP TABLE if exists contents;
DROP TABLE if exists notes CASCADE;
DROP TABLE if exists metadata;

CREATE OR REPLACE TABLE metadata (
  path TEXT PRIMARY KEY NOT NULL,
);

CREATE OR REPLACE TABLE notes (
  note_id TEXT PRIMARY KEY NOT NULL,
  note_title TEXT NULL,
  note_type TEXT NOT NULL,
  path TEXT NOT NULL,
  hash UBIGINT NOT NULL,
  modified TIMESTAMP NOT NULL,
  frontmatterlines INTEGER NOT NULL,
  lines INTEGER NOT NULL,
);

CREATE OR REPLACE TABLE tags (
  tag TEXT NOT NULL,
  note_id TEXT NOT NULL,
  loc TEXT NOT NULL,
  line UINTEGER NULL,
  start_col UINTEGER NULL,
  end_col UINTEGER NULL,
  CONSTRAINT fk_Note FOREIGN KEY (note_id) REFERENCES notes(note_id)
);

CREATE OR REPLACE TABLE authors (
  source TEXT NOT NULL,
  author TEXT NOT NULL,
  CONSTRAINT fk_Source FOREIGN KEY (source) REFERENCES notes(note_id),
  PRIMARY KEY (source, author)
);

CREATE OR REPLACE TABLE parents (
  parent TEXT NOT NULL,
  child TEXT NOT NULL,
  CONSTRAINT fk_Child FOREIGN KEY (child) REFERENCES notes(note_id),
  PRIMARY KEY (parent, child)
);

CREATE OR REPLACE TABLE refs (
  note_id TEXT NOT NULL,
  to_id TEXT NOT NULL,
  loc TEXT NOT NULL,
  line UINTEGER NULL,
  start_col UINTEGER NULL,
  end_col UINTEGER NULL,
  CONSTRAINT fk_FromNote FOREIGN KEY (note_id) REFERENCES notes(note_id)
);

CREATE OR REPLACE TABLE contents (
  note_id TEXT NOT NULL,
  line UINTEGER NOT NULL,
  text TEXT NOT NULL,
  CONSTRAINT fk_Note FOREIGN KEY (note_id) REFERENCES notes(note_id),
  PRIMARY KEY (note_id, line)
);
