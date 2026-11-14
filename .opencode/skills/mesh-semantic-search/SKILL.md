---
name: Mesh Semantic Search
description: Implement and maintain the planned semantic note search feature in mesh.nvim, including DuckDB embeddings, Rust endpoints, Lua picker integration, and validation.
---

# Mesh Semantic Search

Use this skill when implementing or extending the semantic note search feature described in `docs/semantic-search-plan.md`.

## Read first

- `docs/semantic-search-plan.md`
- `AGENTS.md`
- `src/lib.rs`, especially `lua_db`, `update_index`, and the Lua-facing endpoint pattern
- `src/db.rs`, especially `Db::process_files`, `Db::update_notes`, and query helpers
- `src/parse/body.rs` and `src/parse/mod.rs` for parsed note/anchor data
- `lua/mesh-nvim/init.lua`, `lua/mesh-nvim/pick.lua`, and `lua/mesh-nvim/buf.lua`

## Planned scope

- Add an `embeddings` table to `sql/clear.sql`.
- Add Rust types and `Db` methods for embedding storage and search.
- Add local embedding generation behind an optional Cargo feature.
- Add `semantic_search_prompt` and `semantic_search_note` Rust endpoints.
- Add Lua endpoints and a snacks.nvim picker.
- Keep hybrid ranking: semantic, keyword, and graph weights.

## Build and test commands

```bash
nix develop --command bash -lc 'cargo check'
nix develop --command bash -lc 'cargo test --lib'
nix develop --command bash -lc 'cargo build --release'
nix build
```

Use `~/zettel` and `~/.local/share/nvf/mesh/zettel.duckdb` for manual integration testing.

## Implementation constraints

- Keep the Lua-facing API shape `{run, opts}` for action results.
- Do not break existing note indexing or pickers.
- Preserve untracked PDF fixtures in `tests/`.
- Prefer incremental changes by phase from the plan.
- Report PDF annotation errors instead of panicking.
- Keep Nix builds working; Nix skips tests because fixtures are not packaged.

## Data flow

1. `file::Paths::go` detects new, modified, and deleted notes.
2. `Db::update_notes` inserts note data.
3. Embedding generation creates note and anchor chunks.
4. Search embeds the prompt or source note.
5. DuckDB returns candidate embeddings.
6. Rust applies hybrid scoring and filters.
7. Lua displays results and opens selected locations.

## Validation

- Add unit tests for chunk generation and score calculation.
- Run `cargo test --lib`.
- Run a manual picker test against the zettel vault.
- Profile indexing and query latency for medium vaults before enabling HNSW or background indexing.
