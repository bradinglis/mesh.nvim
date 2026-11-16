---
name: Mesh Semantic Search
description: Implement and maintain the planned semantic note search feature in mesh.nvim, including DuckDB embeddings, local Rust embedding, hybrid scoring, Lua picker integration, and validation.
---

# Mesh Semantic Search

Use this skill when implementing or extending semantic note search.

## Read first

1. `docs/semantic-search-plan.md` - the v2 plan, decisions, phases, and spike gates.
2. `docs/refactor-review.md` - blocking prerequisites found in the existing code.
3. `AGENTS.md` - build, test, and architecture notes.

Key code: `src/lib.rs` (`lua_db`, endpoint pattern), `src/db.rs` (`Db::init`, `update_notes`, query helpers, `DbValue`), `sql/clear.sql`, `lua/mesh-nvim/util.lua` (`try` joins `update_thread`), `lua/mesh-nvim/init.lua` (`update`, `update_async`), `lua/mesh-nvim/pick.lua`, `lua/mesh-nvim/buf.lua` (`open`).

## Work in phases

Follow the plan's phase order. Do **Phase 0 spikes** before adding a model dependency, and land the **Phase 1 prerequisites** before storing any embeddings. Do not skip ahead: the backend choice (candle vs ONNX) is made by the **user** from the S1 measurements; stop and report before Phase 3. Embeddings refresh automatically after each save (debounced, one thread). Results jump to the best-matching line via Stage 3 (on-the-fly line embedding, keyword fallback), not per-line storage.

## Hard constraints

- **Schema:** tables are only created by `Db::clear()` on a vault change. New tables go in an idempotent `Db::ensure_schema()` (`CREATE TABLE IF NOT EXISTS`) that runs on every `Db::init`, and `clear.sql` must also drop them.
- **Locking:** every endpoint opens its own DuckDB connection and holds the file lock. Never hold it while embedding. Use plan -> embed (no DB handle) -> short commit per batch.
- **Threads:** `util.try` joins `update_thread`. Embedding runs in its own thread that `util.try` does not join. Thread payloads to the main thread are strings only.
- **Never block the UI:** do not add embedding to the synchronous `M.update()`.
- **Offline builds:** nothing may be downloaded at build time (Nix sandbox). fastembed default features (`ort-download-binaries`) are not allowed; use `default-features = false`. Model weights download at runtime to `stdpath("data")/mesh/models`.
- **Testability:** all logic depends on the `Embedder` trait. Tests use the deterministic fake embedder, never a real model or the network; real-model tests are `#[ignore]`. `nix build` runs the tests.
- **Lua API shape:** keep `{run, opts}` action results. Do not return nullable list items (`DbValue::List` drops `None`, leaving holes).
- **Locations:** results use the `anchors` conventions (1-based line, 0-based columns) so `buf.open` works unchanged.
- **Scope default:** exclude `author` notes unless `filters.type` says otherwise.
- Preserve `tests/*.pdf`; keep `tests/` tracked and in the flake fileset.

## Commands

```bash
nix develop --command bash -lc 'cargo check'
nix develop --command bash -lc 'cargo clippy --lib --tests'
nix develop --command bash -lc 'cargo test --lib'
nix develop --command bash -lc 'cargo build --release'   # required before manual Lua testing
nix build                                                # also runs the tests
nix develop --command nvim --headless -u NONE --cmd 'set runtimepath+=.' -c 'luafile tests/smoke.lua' -c qa
```

Manual data: vault `~/zettel`, database `~/.local/share/nvf/mesh/zettel.duckdb` (691 notes, 1,452 anchors, ~2.9 M chars). Copy the database before experimenting; never run schema experiments on the live file.

## Scoring recap

`score = w_sem*sem + w_kw*kw + w_graph*graph` (prompt 0.6/0.2/0.0; note search 0.6/0.0/0.4). Graph is a noisy-or of link (1.0), parent (0.8), author (0.6), and IDF-weighted shared tags, measured against seed notes (context note, the note itself, or top-3 semantic hits). Redistribute a missing signal's weight pro rata. Keep component scores in each result so weights can be tuned against the golden queries. Chunk text includes the parent note title (book context) in the note header and in section prefixes; chapter sections otherwise often lack the book's vocabulary.

## Definition of done per phase

- Unit tests added for the new pure logic.
- `cargo fmt --check`, `cargo clippy --lib --tests`, `cargo test --lib`, and `nix build` pass.
- `AGENTS.md` and the plan's decision/open-question sections updated.

## Status

- Phase 0 (spikes): done, branch `spike/semantic-backends`. ONNX chosen.
- Phase 1-2 (storage, chunking, prerequisites): done.
- Phase 3 (ONNX embedder, refresh, warm-up): done.
- Phase 4 (search backend): done. Prompt default graph weight 0; note search defaults in `semantic_search.rs`.
- Phase 5 (Lua commands, picker, config): done; snacks UI verified only through a stubbed picker in the smoke test.
- Phase 6 (tuning): weight sweep and profile recorded; parent title added to chunk context, which fixed the outstanding Marx reification miss (set A now 10/10). Open: linear search cost for a medium vault (cache or ANN, deferred), and note-search evaluation. See `docs/semantic-eval.md`.
