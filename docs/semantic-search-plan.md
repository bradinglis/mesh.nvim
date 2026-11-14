# Semantic note search plan

## Goal

Add a semantic relevance search endpoint to mesh.nvim. It should accept either a free-text prompt or an existing note, embed the query locally, compare it against indexed note/anchor embeddings, and open a snacks.nvim picker with ranked notes, anchors, snippets, and source locations.

This is a planning document only. No implementation has been started yet.

## User-facing API

Two Lua entry points are planned:

```lua
-- Free-text prompt, with optional note context for disambiguation.
M.semantic_search(prompt, {
  context_note = "Alpha",   -- optional note id/title
  limit = 20,
  include_notes = true,
  include_anchors = true,
  filters = { type = "note", tags = { "topic" } },
})

-- Use the current or named note as the query.
M.semantic_search_note(note_id, {
  limit = 20,
  include_notes = true,
  include_anchors = true,
})
```

Rust/Lua boundary:

- Rust computes embeddings and returns ranked results.
- Lua builds and displays the snacks picker and opens the selected location.
- The Rust endpoint should return a list of result records, not drive UI directly.

Proposed result record:

```lua
{
  note_id = "Alpha",
  anchor_id = "alpha-introduction", -- nil for note-level results
  title = "Alpha",
  kind = "anchor",                 -- "note" or "anchor"
  path = "/vault/notes/alpha.md",
  line = 12,
  start_col = 0,
  end_col = 42,
  score = 0.87,
  semantic_score = 0.91,
  keyword_score = 0.62,
  graph_score = 0.40,
  snippet = "See [[Beta]] and #topic.",
  tags = { "topic" },
}
```

The snacks picker should show score, kind, title, snippet/tags, and allow opening the selected note at the returned line.

## Example data

Use the existing vault for development and manual testing:

- Notes: `~/zettel`
- Database: `~/.local/share/nvf/mesh/zettel.duckdb`

This keeps the plan aligned with the user's current Neovim/NVF setup.

## DuckDB schema changes

Add an embeddings table alongside the existing notes/tags/refs/anchors tables:

```sql
CREATE TABLE IF NOT EXISTS embeddings (
    note_id TEXT NOT NULL,
    anchor_id TEXT,
    chunk_kind TEXT NOT NULL,      -- 'note' or 'anchor'
    text TEXT NOT NULL,
    vector FLOAT[] NOT NULL,
    content_hash UBIGINT NOT NULL,
    model TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    PRIMARY KEY (note_id, anchor_id)
);
```

Notes:

- `anchor_id` is `NULL` for note-level embeddings.
- `vector` length must match the configured model dimension.
- A content hash avoids recomputing embeddings for unchanged notes.
- An HNSW index may be added later through DuckDB's VSS extension if available. The first implementation can use brute-force cosine similarity via DuckDB list functions and optimize after profiling.

## Chunking strategy

Use the existing parsed note structure:

- One `note` chunk per note:
  - title/aliases
  - note type
  - tags
  - source parents and authors
  - a truncated or full body, depending on model context limits
- One `anchor` chunk per heading:
  - heading title
  - body text until the next heading
  - anchor id and location

This reuses `note::Note`, `note::Anchor`, and `ContentLine` data already produced by the parser.

## Local embedding model

The user requested local embedding generation in Rust. Proposed implementation:

- Add an optional Cargo feature, e.g. `semantic`, defaulting to enabled.
- Use a Rust embedding crate such as `fastembed` with a small default model (`BAAI/bge-small-en-v1.5`, 384 dimensions).
- Download/cache model files under `stdpath("data")/mesh/models`.
- Expose a small internal module, e.g. `crate::embeddings`, with:

```rust
pub fn embed_texts(texts: &[&str]) -> Result<Vec<Vec<f32>>>
pub fn model_name() -> &'static str
pub fn vector_dim() -> usize
```

Open questions:

- Whether `fastembed`'s dependency/build weight is acceptable.
- Whether model download should be lazy, explicit, or happen during `update_index`.
- Whether to support multiple models or lock to one dimension for schema simplicity.

## Index lifecycle

Embeddings should be maintained during normal indexing:

1. `Db::process_files()` returns added/modified/deleted notes.
2. `Db::update_notes()` already has the parsed `note::Note` values and removed paths.
3. After inserting notes/tags/refs/anchors/contents, compute embeddings for added/modified notes.
4. Delete embeddings for removed note ids.
5. Store content hashes and skip unchanged chunks.

Because embedding generation can be slow, indexing should eventually move to a background thread or expose progress. The existing worker-thread path (`update_async`) is the natural place for this. The synchronous `M.update()` may need to become non-blocking for medium vaults.

## Search and ranking

The Rust search method should:

1. Embed the prompt or source note.
2. Optionally embed the context note and combine vectors (average is a reasonable first approach).
3. Query candidate embeddings from DuckDB.
4. Compute semantic similarity.
5. Add keyword overlap score.
6. Add graph score from tags, links, references, source parents, and authors.
7. Return top `limit` results.

Proposed hybrid score:

```text
score =
  semantic_weight * cosine_similarity +
  keyword_weight * normalized_keyword_overlap +
  graph_weight * normalized_graph_overlap
```

Suggested initial weights:

- semantic: 0.65
- keyword: 0.20
- graph: 0.15

Graph weighting can use:

- shared tags
- direct links/references
- shared source parents/authors
- same note type
- recency as a small tie-breaker

Filters should support vault, note type, tags, and possibly date ranges.

## Lua integration

Add to `lua/mesh-nvim/`:

- `semantic.lua` or functions in `init.lua`
- `completion/` is not needed for search
- snacks picker construction similar to `pick.lua`

Lua responsibilities:

- Read config and current vault.
- Call Rust endpoints.
- Convert results to picker items.
- Open selected location using existing `buf.open`/`rfun.run("open", ...)`.
- Show indexing/search progress with `vim.notify`.

Config additions:

```lua
semantic = {
  enabled = true,
  model = "bge-small-en-v1.5",
  limit = 20,
  weights = {
    semantic = 0.65,
    keyword = 0.20,
    graph = 0.15,
  },
  include_notes = true,
  include_anchors = true,
}
```

## Rust API additions

Planned `mesh` exports:

```rust
semantic_search_prompt(
  notes_dir,
  db_file,
  prompt,
  context_note: Option<String>,
  opts
) -> Vec<SearchResult>

semantic_search_note(
  notes_dir,
  db_file,
  note_id,
  opts
) -> Vec<SearchResult>
```

`Db` additions:

```rust
fn ensure_embedding_schema(&self) -> Result<()>
fn update_embeddings(&self, notes: &[note::Note], removed: &[String]) -> Result<Vec<String>>
fn search_embeddings(&self, query: &[f32], opts: SearchOpts) -> Result<Vec<SearchResult>>
```

`SearchOpts` should include limit, filters, include flags, and weights.

## Implementation phases

1. Schema and storage
   - Add `embeddings` table to `sql/clear.sql`.
   - Add Rust structs and DB methods.
   - Verify DuckDB list/vector operations.

2. Local embeddings
   - Add optional Cargo feature and embedding crate.
   - Implement model download/cache.
   - Add embedding text generation for notes and anchors.

3. Indexing integration
   - Call embedding updates from `Db::update_notes`.
   - Handle deletions and content-hash skips.
   - Report progress/errors.

4. Search backend
   - Implement cosine similarity query.
   - Add keyword and graph scoring.
   - Add filters and result serialization.

5. Lua and picker
   - Add endpoints and config.
   - Build snacks picker.
   - Open selected note/anchor location.

6. Performance and UX
   - Profile medium vaults.
   - Consider HNSW index and background indexing.
   - Add caching and cancellation if needed.

## Testing plan

- Unit tests for chunk text generation.
- Unit tests for hybrid score calculation.
- Integration test against `~/zettel` and `~/.local/share/nvf/mesh/zettel.duckdb`.
- Manual picker test for note and prompt search.
- Performance test for medium vault indexing and query latency.

## Risks and open questions

- Local model dependency size and build time.
- Model download behavior in offline environments.
- DuckDB vector extension availability and portability.
- Whether anchor-level chunks are too numerous for medium vaults.
- Whether semantic search should update embeddings synchronously or in the background.
- Whether graph weights should be user-configurable or fixed initially.
- Whether to support multiple embedding models with different vector dimensions.

## Recommended first implementation

Start with:

- one fixed local model (`bge-small-en-v1.5`, 384 dimensions)
- note + anchor chunks
- brute-force cosine similarity in DuckDB
- hybrid scoring with fixed weights
- synchronous indexing in the existing worker thread
- snacks picker with open-at-location behavior

This keeps the first version understandable while leaving clear extension points for HNSW, background indexing, and configurable models.
