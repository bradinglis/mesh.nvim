# Semantic note search plan (v2)

Status: plan only, no implementation yet. Read `docs/refactor-review.md` first; several of its blocking items are prerequisites here.

## Goals

1. A Lua endpoint that takes a **string** (optionally with a **context note**) and returns a picker of relevant notes and anchors with their locations.
2. A second endpoint that takes the **current note** (or a given note id) as the query.
3. Ranking is **hybrid**: semantic similarity + keyword match, boosted by graph signals (tags, links, references, authors, parents).
4. Embeddings are generated **locally in Rust** and stored in the vault's DuckDB file.
5. Results open at the right file and line, using the same picker/open conventions as the existing pickers.

## Decisions (from review with the user)

| Topic | Decision |
| --- | --- |
| Inference backend | **Spike both** candle (pure Rust) and ONNX (Phase 0); the spike reports measurements and **the user chooses** (no automatic winner) |
| Embedding refresh | **Automatic** after each successful save/index (background, resumable) plus an explicit command |
| Model weights | Download at runtime into `stdpath("data")/mesh/models`; not built into Nix |
| Query execution | Synchronous Lua call; model **warmed in a background thread** at startup |
| Default scope | All types **except authors**; `filters.type` overrides |
| Vault size target | Medium (current vault is smaller, see below) |
| Result granularity | Notes and anchors, with the jump target refined to the **best-matching line** inside the chunk (see Stage 3) |

## Measured baseline (`~/zettel`, `~/.local/share/nvf/mesh/zettel.duckdb`)

| Metric | Value |
| --- | --- |
| Notes | 691 (487 source, 73 note, 131 author) |
| Anchors (headings) | 1,452; every note has at least one |
| Body text | ~2.9 M chars in 23.8 k lines; avg 4.3 k chars/note, max 49 k |
| Tags | 1,724 uses, 668 distinct (sparse) |
| Refs | 1,065 in-text links, 493 author, 305 parent, 245 frontmatter references |

Consequences:

- Expect roughly 2.5-3.5 k chunks. Brute-force `list_cosine_similarity` over that is milliseconds. **No HNSW / VSS extension is needed** (also avoids a network `INSTALL`). Verified: `list_cosine_similarity` is available in core DuckDB 1.5.
- Notes can far exceed a 512-token model window, so one vector per note is not enough: chunk by section with windows.
- Tags are near-unique, so tag overlap must be IDF-weighted or it will almost never fire. Links/author/parent edges are the stronger graph signal.

## Architecture

```
Lua (init.lua / semantic.lua / pick.lua)
  | mesh.semantic_search_prompt / _note / update_embeddings / warm_model / embedding_status
Rust cdylib
  embeddings.rs   Embedder trait, model loading (static, shared), chunk jobs
  chunking.rs     sections -> windows -> ChunkJob (pure, unit-tested)
  semantic.rs     candidate retrieval + scoring + result assembly (pure scoring fns)
  db.rs           ensure_schema, embedding CRUD, candidate queries
DuckDB            embeddings + embedding_state tables, existing notes/tags/refs/anchors/contents
```

### Types (stronger typing, per the earlier goal)

```rust
trait Embedder: Send + Sync { fn dim(&self) -> usize; fn model_id(&self) -> &str;
                              fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>; }
enum ChunkScope { Note, Anchor }          // stored as 'note' / 'anchor'
struct ChunkJob { note_id, anchor_id, part, scope, text, text_hash, line }
struct SearchOpts { limit, filters: Filters, weights: Weights, include: Scope, graph_seeds: bool, refine_line: bool }
struct Weights { semantic, keyword, graph }   // renormalised when a signal is absent
struct Hit { note_id, anchor_id: Option<_>, scope, title, heading, path, line, chunk_line, start_col, end_col,
             score, semantic, keyword, graph, snippet, tags }
enum Query { Prompt { text, context_note: Option<String> }, Note(String) }
```

`Embedder` is a trait so unit tests use a deterministic fake (hashed bag-of-words) and **never load a model or touch the network**. Real-model tests are `#[ignore]`. This keeps `nix build` (which runs tests) offline-safe.

## Storage

Created by an idempotent `Db::ensure_schema()` on every `Db::init` (the existing `clear.sql` path only runs on a vault change; see review item 1):

```sql
CREATE TABLE IF NOT EXISTS embedding_state (
  model TEXT NOT NULL, dim INTEGER NOT NULL, schema_version INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS embeddings (
  note_id   TEXT NOT NULL,
  scope     TEXT NOT NULL,              -- 'note' | 'anchor'
  anchor_id TEXT NOT NULL DEFAULT '',   -- '' for note-level chunks
  part      INTEGER NOT NULL DEFAULT 0, -- window index within the section
  line      UINTEGER NULL,              -- absolute 1-based line where the chunk starts
  text      TEXT NOT NULL,
  text_hash UBIGINT NOT NULL,
  vector    FLOAT[] NOT NULL,
  PRIMARY KEY (note_id, scope, anchor_id, part)
);
```

- No foreign key (derived data). Orphans are removed with `DELETE ... WHERE note_id NOT IN (SELECT note_id FROM notes)` at the start of each embedding run.
- A row is **stale** when its `text_hash` differs from the freshly built chunk, or `embedding_state.model/dim` differs from the loaded model. A model change clears the table.
- `clear.sql` also drops both tables (vault change).

## Chunking (built from the DB, not the parser)

Building chunks from `notes`/`anchors`/`contents`/`tags`/`refs` means existing databases can be backfilled without re-parsing, and chunking is independent of the indexing pass.

- **Note chunk** (one per note): `title`, type, tags, author titles (from `refs.kind='author'` joined to `notes`), then the start of the body up to the model limit. Author notes get **only** this chunk.
- **Anchor chunks**: lines of `contents` from an anchor's `line` to the next anchor's `line`. Prefix each chunk with `Note title > heading`. Sections over ~1,500 chars (~350 tokens) are split into windows on paragraph boundaries with ~150 chars overlap; `part` indexes the window.
- Note-level score for ranking = max over the note's chunks.

## Embedding pipeline (lock-friendly, resumable)

Embedding is minutes of CPU. It must not hold the DuckDB file lock (review item 2).

1. **Plan** (short read): build `ChunkJob`s, left-anti-join against `embeddings` to find missing/stale ones.
2. **Embed** (no DB handle): batches of 32-64 via the shared static model.
3. **Commit** (short write): one transaction per batch (delete old rows for those keys, insert new). Progress is durable, so an interrupted run resumes.

`mesh.update_embeddings(dir, db, opts)` returns counts and any per-batch errors (a good use of the existing `Results` type). It runs in its own worker thread (`vim.uv.new_thread`); `util.try` must **not** join that thread. Commits retry with short backoff on lock errors (behaviour to be confirmed in spike S3).

Triggers: automatically after each successful structural `update_index` (so after every `BufWritePost` and at startup), and explicitly via a command. Because planning is a cheap anti-join, a save that changed nothing embeds nothing. Saves are debounced and at most one embedding thread runs; a save during a run just marks it dirty for one more pass. It is never part of the synchronous `M.update()`.

## Search

`semantic_search_prompt(notes_dir, db_file, prompt, context_note?, opts)` and `semantic_search_note(notes_dir, db_file, note_id, opts)`.

**Query vector**
- Prompt: `embed(prompt)`. With a context note: `normalize(0.7 * embed(prompt) + 0.3 * note_vector)`, where `note_vector` is read from the DB (no re-embedding).
- Note: the note's stored note-level vector. The note and its own anchors are excluded from results.

**Stage 1 - candidates** (SQL)
- Top 100 by `list_cosine_similarity(vector, ?)`.
- Top 50 by keyword match: query terms (lowercased, stop-words removed) counted with `contains(lower(text), term)`. DuckDB's FTS extension is **not** used (needs a network `INSTALL`).
- Union.

**Stage 2 - scoring** (pure Rust functions, unit-tested)

```
score = w_sem * sem + w_kw * kw + w_graph * graph        (defaults 0.6 / 0.2 / 0.2)
sem   = clamp(cosine, 0, 1)
kw    = BM25-lite over query terms (idf from the chunk corpus), normalised by the candidate max
graph = 1 - (1-link)(1-parent)(1-author)(1-tags)         (noisy-or, each term in [0,1])
```

Graph is measured against **seed notes**: the context note, or the note itself for note search, or (prompt only) the top 3 semantic hits (pseudo-relevance feedback; `graph_seeds=false` disables it). Explicit `#tags` in the prompt also seed the tag term.

| Signal | Source | Value |
| --- | --- | --- |
| direct link / reference either direction | `refs` kinds `intext_link`, `frontmatter_reference` (map `to_anchor` -> note through `anchors`) | 1.0 |
| parent/child source | `refs` kind `parent` | 0.8 |
| shared author | `refs` kind `author` | 0.6 |
| shared tags | IDF-weighted Jaccard over `tags` | 0..1 |

When a signal is unavailable (no seeds, no tags) its weight is **redistributed** pro rata so scores stay comparable.

**Stage 3 - best-matching line** (top results only, no extra storage)
For the top `limit` hits, split the chunk into lines/sentences and pick the jump target:
1. Embed those lines on the fly with the warm model (small batch: ~10 hits x ~10 lines) and take the highest cosine to the query vector; if the model is unavailable or too slow, fall back to
2. the line with the most query-term matches, else
3. the chunk start.
`Hit.line` is the chosen line (absolute 1-based, derived from the chunk's `line` + offset); `Hit.chunk_line` keeps the chunk start. Blank lines, headings-only lines and frontmatter lines are skipped. Budget: Stage 3 adds <= 100 ms warm (measured in S1); `opts.refine_line = false` disables it.

**Assembly**
- Keep the best chunk per anchor; aggregate a note row from its best chunk.
- Default mixed list ordered by score; a note row is dropped if its best chunk is an anchor already listed. `opts.group = "anchor" | "note" | "mixed"`.
- Filters: `type` (default excludes `author`), `tags` (any), `exclude_ids`.
- Each `Hit` carries `path`, `line`, `start_col`, `end_col` (same conventions as `anchors`, so `buf.open` works unchanged), the three component scores, tags and a snippet.

## Lua API

```lua
local mesh = require("mesh-nvim")

-- string query; prompts with vim.ui.input when prompt is nil
mesh.semantic_search(prompt, {
  context_note = "Alpha" | true,    -- note id, or true for the current buffer's note
  limit = 20, group = "mixed",
  filters = { type = { "source", "note" }, tags = { "topic" } },
})

-- query by a note; defaults to the current buffer's note
mesh.semantic_search_note(note_id, { limit = 20 })

mesh.update_embeddings()            -- background; progress via vim.notify
```

- New `lua/mesh-nvim/semantic.lua` wraps the calls with `util.try`, and `pick.from_semantic(hits, title)` builds the snacks picker (score, scope icon, title, heading, snippet; file preview at `line`; `confirm` -> `buf.open{path,row,col}`).
- If the Rust build lacks the feature (`mesh.semantic_search_prompt == nil`), Lua notifies instead of erroring.
- `warm_model()` is called once from `setup` in a worker thread. A search before the model is ready shows "model loading".
- Config: `semantic = { enabled, limit, weights, model, group }` in `config.lua`.

## Build and packaging

- Cargo feature `semantic` (on by default; can be turned off to keep a lean build). Gate all model code behind it.
- Model files are downloaded on first use (`hf-hub`) with a clear notification and an actionable offline error. Nothing is fetched at build time, so the Nix sandbox stays offline.
- **Nix:** whichever backend wins must build offline in the sandbox. fastembed's *default* features (`ort-download-binaries`) do **not**; the plan uses `default-features = false` with either `bert-text` (candle) or `onnx` + ort `load-dynamic` with onnxruntime from nixpkgs.
- Keep `nix build` running the tests; model-dependent tests are `#[ignore]`.

## Phases

0. **Spikes** (decide before building)
   - **S1 backend:** candle (`bert-text`) vs ONNX (`load-dynamic`), models `bge-small-en-v1.5` vs `all-MiniLM-L6-v2`. Measure: builds in `nix build`; build-time and `libmesh.so` size delta; full index time for ~3 k chunks; warm query latency; Stage 3 line-refinement latency (~100 short texts); model load time; quality on ~10 hand-written queries over `~/zettel`. Gates: full index <= 10 min, warm query embed <= 150 ms, Stage 3 <= 100 ms, model load <= 3 s. **Output is a measurements table; the user picks the backend/model.** Do not start Phase 3 until they do.
   - **S2 threads:** confirm the model static is shared between the main Lua state and a `vim.uv` thread (same loaded `.so`), so warm-up helps the main thread.
   - **S3 locking:** what happens when a second `Connection::open` targets the file while another connection is writing, in one process. Decide retry/backoff vs `ATTACH`.
   - **S4 SQL:** time `list_cosine_similarity` over 3 k x 384 and the keyword query.
   - **S5 download:** `hf-hub` progress reporting and offline failure message.
1. **Prerequisites** (review items 1-5): `ensure_schema`, separate embedding thread + lock retry, `tests/smoke.lua`, nullable-list handling.
2. **Storage + chunking** with the fake `Embedder`: schema, `ChunkJob` build, stale detection, orphan cleanup. Fully unit-tested, no model.
3. **Model integration:** real `Embedder`, warm-up, `update_embeddings` (plan/embed/commit), progress.
4. **Search:** candidate SQL, scoring functions, seeds, assembly, both Rust endpoints, `FromLua` for `SearchOpts`.
5. **Lua:** `semantic.lua`, picker, config, commands/keymaps docs, extend `tests/smoke.lua`.
6. **Hardening:** profile on `~/zettel`, tune weights against the query set, update `AGENTS.md` and the skill.

## Testing

- Pure unit tests: chunking (windows, overlap, author-only note chunk), stale detection, noisy-or graph score, weight redistribution, BM25-lite, result dedupe.
- DB tests on a temp vault with the fake embedder: backfill an existing DB without `embeddings`, model-change invalidation, orphan cleanup, interrupted run resumes.
- Golden query set in `docs/semantic-eval.md` (10 queries with expected notes in `~/zettel`) used for weight tuning; run manually, not in CI.
- `tests/smoke.lua`: index temp vault -> `update_embeddings` (fake or tiny model) -> both searches -> open a hit and assert the cursor line.
- Nix build keeps passing with no network access.

## Risks

| Risk | Mitigation |
| --- | --- |
| Backend adds large build/runtime weight or breaks the Nix sandbox | Phase 0 spike with hard gates; feature flag |
| First index too slow on CPU | Resumable batches, background thread, smaller model, measured in S1 |
| DuckDB lock contention during embedding | Phased pipeline with short commits, retry/backoff (S3) |
| Weak keyword/graph signal on sparse tags | IDF-weighted tags, link/author/parent edges, weight redistribution |
| Schema drift on existing databases | `ensure_schema` + `embedding_state` version/model row |
| Quality unknown until tried | Golden query set, tunable weights, expose component scores in the picker |

## Open questions

- Which model/backend to ship: decided by the user after the S1 measurements.

Resolved: automatic refresh after each save (debounced, single thread); line-level targeting via Stage 3 rather than indexing every line.
