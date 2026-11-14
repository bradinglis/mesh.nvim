# Review of `review/refactor-cleanup`: required changes and follow-ups

Scope: the refactor commits `99a02e0` and `63efa1d` (and the code they touch), reviewed against the semantic search goals in `docs/semantic-search-plan.md`.

Legend: **Applied** = already fixed on `review/refactor-cleanup`. **Blocking** = must land before or with semantic search. **Recommended** = worth doing, not blocking. **Note** = behaviour change reviewers should know about.

## Verification status

| Check | Result |
| --- | --- |
| `cargo fmt --check`, `cargo clippy --lib --tests` | clean |
| `cargo test --lib` | 15/15 pass |
| `nix build` (now running the tests too) | passes, 15/15 |
| Headless Lua smoke test (index + thin notes + tags + backlinks on a temp vault) | passed once, ad hoc, **not in the repo** |
| Lua flows (`annotations`, `fill_from_boox`, `create`, pickers, completion) | syntax-checked and require-checked by the Nix plugin check only; **not exercised in a real Neovim session** |
| Building with stable Rust | **unverified** (no `#![feature]` remains, but the flake pins nightly; edition 2024 and let-chains need >= 1.88) |

## Applied

- **Nix now runs the tests.** `tests/` is tracked, so it is in the `src` fileset and `doCheck = false` is gone. Verified by building with tests enabled (`63efa1d`). `AGENTS.md` updated to match.

## Blocking for semantic search

1. **Schema is only created in `Db::clear()`.**
   `Db::init` calls `clear()` (which runs `sql/clear.sql`) only when `metadata.path` is missing or differs from the vault dir. An `embeddings` table added to `clear.sql` would never be created in an existing database such as `~/.local/share/nvf/mesh/zettel.duckdb`.
   Required: an idempotent `Db::ensure_schema()` run on every `init` (`CREATE TABLE IF NOT EXISTS`), plus a schema/model version row so a model or dimension change invalidates stored vectors. `clear.sql` must also `DROP` the new tables on a vault change.

2. **A long write blocks every picker.**
   `util.try` joins `update_thread` before running any query, and each endpoint opens its own DuckDB connection (`Db::init`), which takes the file lock. Embedding a vault is minutes of work, so it must not hold the lock or be joined like the structural update.
   Required: a phased embedding pipeline (read chunks to embed -> embed with **no** DB handle -> write one short batch), a separate embedding thread that `util.try` does not join, and a retry/backoff on lock errors. Whether two `Connection::open` calls on one file in one process fail or share an instance is **unverified** (spike S3 in the plan).

3. **`M.update()` is synchronous and runs on `setup` and every `BufWritePost`.**
   It blocks the UI today. Embeddings must not be added to it. Add a separate `update_embeddings` endpoint driven from a worker thread. (`update_async` already exists but is unused; its channel only forwards `tostring()` values, which is fine for log strings but not for table payloads.)

4. **`DbValue::List` drops `None` items, leaving holes** (`impl IntoLua for DbValue`: `if let DbValue::None = x {} else { t.set(i + 1, x) }`). Lua `ipairs` and `#` then stop early. Search results are built from non-null values so this is safe, but it is a trap for any list that can contain nulls. Required: do not return nullable list items from new endpoints; consider compacting in `IntoLua`.

5. **No Lua-side test harness.** The refactor changed most Lua modules and only syntax/require checks cover them. Semantic search adds a picker and thread logic. Required: commit the headless smoke script (index a temp vault, call each endpoint) as `tests/smoke.lua`, run by a documented command, and extend it for the new endpoints.

6. **Location conventions must be pinned down.**
   `anchors.line` is the absolute 1-based file line of the heading (**verified** against four random anchors in `~/zettel`: `sed -n "${line}p"` returns the heading); `start_col`/`end_col` are 0-based byte offsets; `anchors.anchor_id` is globally unique and, for a level-1 heading, equals the note id (**verified**: 691 of 691 notes have exactly one such anchor, 1,452 anchors total). Search results must use the same conventions as `get_loc` / `buf.open` (row 1-based, col 0-based). Still to verify in the smoke test: open a result and assert the cursor line.

## Recommended

- **Unused code kept on purpose:** `results.rs` (`Results`), `trier.rs` (`Tryer`), `MyResult::i`, `Ref.alias` (`#[allow(dead_code)]`), the `Deref` impls on `Table`/`ReturnValue`. `Results` is a good fit for collecting per-batch embedding errors; otherwise decide to delete.
- **`create.lua` double update.** `rfun.try(..., true, viz)` updates the index first, then `util.try(update_index)` runs again after creation. Only the second is needed unless `create_new` relies on a fresh parent row; confirm, then drop one.
- **Cargo profile.** `lto = "fat"` + `codegen-units = 1` gives ~2 min incremental release builds. A candle/ONNX dependency will lengthen this; consider a `dev-release` profile (thin LTO) for iteration.
- **`flake.nix`:** `duckdb.lib` is listed under `nativeBuildInputs` as well as `buildInputs`; harmless but redundant.

## Behaviour changes to note in review

- `update_notes` no longer aborts on the first bad reference/tag; it records `Error: ...` messages and continues (it used to `?` out and discard the whole batch).
- Frontmatter lines with unknown keys or no `": "` are ignored (the original code built an error and then discarded it; the refactor makes that explicit).
- Tags, links, contents, authors and parents are now inserted in document order (the original `pop()` loops reversed them). Any code relying on reversed order would change; none was found.
- Appenders flush at the end instead of `notes` flushing first. There are no cross-table constraints at append time, and the DB test passes.
- `cursor_action` Rust parameters were renamed `(content, line, col)` and `Note::get_ref(line, col)` was fixed to match what `buf.lua` already sent (`row, col`). The old signature swapped the meaning.
- `get_annotations` now returns errors to Lua instead of panicking; `M.annotations` only renames the PDF on success.
- `get_note_anchors` / `get_thin_notes` SQL had trailing commas before `from`; removed.
- `pick.lua`: truncation now keeps the start of long text (it used to keep the tail); `M.vault` lists vaults by name and skips the `default` pointer.
- `config.set_opts` accepts a vault given as a plain directory string and tolerates a missing `name`.
