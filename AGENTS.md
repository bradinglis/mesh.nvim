# mesh.nvim

Neovim plugin for a Markdown note vault: Rust `cdylib` (`mesh`) indexes notes into DuckDB; Lua under `lua/mesh-nvim/` provides UI, pickers, and completion. Depends on `snacks.nvim` and `blink.cmp`. The same crate also builds a `mesh` CLI (see `docs/cli.md`).

## Build & dev

- Toolchain is pinned to nightly Rust through the flake: `nix develop` (or rely on `.envrc` + direnv). The crate no longer uses unstable feature gates, but use the flake's pinned toolchain for reproducible builds.
- DuckDB is linked dynamically (crate feature `bundled` is disabled). `libduckdb` must be available at build and runtime; the flake provides it.
- Lua loads the library from `target/release/libmesh.so` (`util.get_paths().release_dir`). After Rust edits run `cargo build --release`, not a debug build.
- Build plugin via Nix: `nix build` (copies `libmesh.so` into the vim plugin tree). Nix builds run `cargo test --lib` too, so `tests/` must stay tracked in git (flakes ignore untracked files) and listed in the `src` fileset in `flake.nix`.
- Quick compile check: `cargo check`.

## Testing

- `cargo test --lib` runs the Rust unit tests across core modules (plus the CLI render/JSON/args helpers). Run annotation tests with `cargo test --lib highlights::tests::test_one`.
- `cargo test --test cli` runs the CLI integration tests (`tests/cli.rs`); they build a throwaway vault and need no model or network.
- PDF fixtures live in `tests/*.pdf`; keep them present when running annotation tests.
- Last green baseline before the annotations refactor: `a6746c5` (`origin/main`).
- Last commit before this cleanup refactor: `6c2d2c7`.

## Architecture

- Rust entry: `src/lib.rs` exports the `mesh` Lua module; every function takes `(notes_dir, db_file, ...)`. The Lua-free core lives in `src/api.rs`; `lib.rs` bindings and the CLI are thin adapters over it, so their behaviour cannot drift.
- CLI: `src/main.rs` calls `mesh::cli::run` (`src/cli/`). Hand-rolled arg parser (`args.rs`), table renderer (`render.rs`), and a minimal JSON/JSONL writer (`json.rs`). Read commands open the database through `Db::open_read_only`.
- Lua entry: `lua/mesh-nvim/init.lua` (`setup(opts)`). `lua/mesh-nvim/binary.lua` loads `mesh` from `target/release`.
- DB: one DuckDB file per vault at `stdpath("data")/mesh/<vault>.duckdb`; schema in `sql/clear.sql`; read queries in `sql/notes_details.sql`.
- Notes: recursively scans the vault for `.md`, skipping `.git` and `templates`. Hashing uses `rapidhash`.
- Vault config: `vaults` table plus a `default` key that may be a vault name (string) or a vault table. The shipped default (`vaults.default = "none"`) is a placeholder and must be overridden.
- Skills: project-specific implementation guidance lives under `.opencode/skills/`, including `mesh-semantic-search` for the planned semantic search feature.
- Planned work: semantic search is specified in `docs/semantic-search-plan.md` (v2); prerequisites and review findings for the existing code are in `docs/refactor-review.md`. Read both before touching the DB schema, threading, or pickers.
- Stronger types: `NoteType`, `RefKind` (`intext_link`, `frontmatter_reference`, `author`, `parent`), and `Location` (`frontmatter`/`body`) replace stringly typed fields in the Rust core.

## CLI

- `mesh` (built from the same crate; `packages.mesh-cli` / `apps.mesh` in `flake.nix`) exposes the library operations to scripts and agents. Docs: `docs/cli.md`.
- Vault/DB resolve from `--vault`/`MESH_VAULT` and `--db`/`MESH_DB`; the DB defaults to `$XDG_DATA_HOME/mesh/<vault>.duckdb`.
- Output is a table on a TTY and JSON when piped; `--format table|json|jsonl`, `--json`, `--jsonl`, `--pretty`. Data goes to stdout, progress/errors to stderr. Exit codes: 0 success, 1 runtime error, 2 usage error.
- Read commands use `Db::open_read_only`, which reports a vault mismatch instead of clearing the file. Write commands (`update`, `embed`) use `Db::init` and need a vault.
- No `serde_json`/`clap`: JSON is hand-written (drops `null` list items to match Lua) and args are parsed by hand to keep startup fast.

## Gotchas

- `cargo build` (debug) compiles but the plugin will not load it; use `--release`.
- `result` is a Nix build-output symlink; `target/` and `.direnv/` are ignored.
- `get_annotations` in `src/lib.rs` now propagates PDF errors instead of panicking; Lua reports the error and leaves the PDF in place.
- Note IDs are derived only from title words starting with an uppercase letter (`ToNoteId`); lowercase words are dropped.
- Completion providers require `blink.cmp`; they are injected for markdown buffers in `completion/init.lua`.
- `lua/mesh-nvim/find.lua` uses `"source"` (not `"sources"`) as the note type for `M.sources()`.
- `lua/mesh-nvim/rfun.lua` no longer has the `vim.notiy` typo; unknown actions are reported instead of silently failing.
- `lua/mesh-nvim/binary.lua` prepends its release directory to `package.cpath` instead of replacing it.
- `lua/mesh-nvim/util.lua` caches `get_paths()` results.
- Completion provider boilerplate is shared through `lua/mesh-nvim/completion/init.lua`.
- Schema is created only by `Db::clear()` (run when the stored vault path is missing or differs). Editing `sql/clear.sql` does not change existing databases; new tables need an idempotent `CREATE TABLE IF NOT EXISTS` step in `Db::init`.
- Every endpoint opens its own DuckDB connection (file lock) via `Db::init`, and `util.try` joins `update_thread` first, so a long-running write blocks all pickers. `M.update()` is synchronous (setup and `BufWritePost`). `Db::open_read_only` (CLI reads) takes the same process-wide gate, so a process still has at most one live `Db`.
- `DbValue::List` drops `None` items, leaving holes in the Lua table (`ipairs`/`#` stop early); avoid nullable list items in new endpoints.
- `anchors.line` is the absolute 1-based heading line, columns are 0-based, and `anchor_id` is globally unique (a level-1 heading's anchor id equals the note id).
- Lua has no automated tests; the Nix plugin check only requires each module. DuckDB core provides `list_cosine_similarity`; the `vss`/`fts` extensions are not installed and must not be relied on.

## Semantic search runtime

- ONNX Runtime is loaded dynamically (`ort` `load-dynamic`). The dev shell exports `ORT_DYLIB_PATH`; the Nix plugin writes `lua/mesh-nvim/runtime.lua` with the pinned path (nixpkgs `onnxruntime` 1.27.1 via `flake.lock`; ort needs API >= 17). Elsewhere, set `ORT_DYLIB_PATH` before loading the plugin.
- Model weights (`BAAI/bge-small-en-v1.5`) download on first use to `<db dir>/models` (`stdpath("data")/mesh/models`). Nothing is fetched at build time.
- Embedding refresh (`lua/mesh-nvim/embed.lua`) runs in its own thread, is never joined by `util.try`, and runs after saves (debounced). Warm-up runs at setup.
- `tests/smoke.lua` downloads the model only when `MESH_SMOKE_MODEL=1`; `cargo test --lib` uses a fake embedder and needs no network.

## Search and database handles

- `Db::init` takes a process-wide gate that is held until the handle drops, so at most one `Db` exists per process. Two live connections to one file crashed a query after a write through the other (SIGSEGV, reproduced in tests). Never keep a `Db` alive across code that opens another one. `update_embeddings` drops its handle before the long embedding loop; keep it that way.
- Search: `semantic_search::search` (candidates: top 100 by cosine, top 50 by keyword; defaults: prompt 0.6 semantic + 0.2 keyword + 0.0 graph; note search 0.6 semantic + 0.4 graph; missing signals redistributed). Graph uses link/parent edges, shared authors and IDF-weighted tags against seed notes; a note never counts as its own seed.
- Chunks: one note-level chunk (title, type, tags, authors, parent title, then the body start) plus one per heading-section window. Section prefixes include the parent note title, so a book chapter carries the vocabulary of the work it belongs to; changing a parent title re-embeds its sections on the next refresh via `text_hash`.
- Keyword line targeting (Stage 3) is keyword-only; there is no on-the-fly line embedding.
- Evaluation notes and measured recall are in `docs/semantic-eval.md`.

## Semantic search front end

- Commands (created in `setup`): `:MeshSearch[!] [prompt]` (bang uses the current note as context), `:MeshSearchNote [note id]`, `:MeshEmbed` (background refresh). Entry points: `require("mesh-nvim").semantic_search`, `.semantic_search_note`, `.refresh_embeddings`, `.embed_status`.
- `lua/mesh-nvim/semantic.lua` holds the options and the pure item builder (no snacks at load time, so headless tests can use it). `pick.from_semantic` builds the snacks picker; confirm opens the note at the hit line via `buf.open`. `pick.set_string_width` truncates to an exact display width (ellipsis included) so columns stay aligned.
- Config: `semantic = { enabled, limit, group, refine_line, graph_seeds, weights, max_per_note, max_per_author, high_score }`, deep-merged in `config.set_opts`. `weights` defaults to nil so each query type keeps its own default.
- Result caps (applied while filling the list, highest score first): `max_per_note` (default 2) repeats of one note; `max_per_author` (default 2) child sources (a `source` note with a parent) per author, unless a hit scores at or above `high_score` (default 0.85). `0` disables either cap.
- Embedding progress: `lua/mesh-nvim/progress.lua` draws a throttled bar on the message line from `update_embeddings`'s `(done, total)` callback; `embed.status()` (exposed as `mesh-nvim.embed_status`) returns a short statusline string. `setup` warms the model and then refreshes, so an interrupted first index resumes at startup.
- Default weights: prompt search semantic 0.6 / keyword 0.2 / graph 0.0; note search semantic 0.6 / keyword 0 / graph 0.4 (not yet measured). Graph 0.2 is within noise of the prompt default after the parent-title chunk fix, so do not treat "graph hurts" as settled; see `docs/semantic-eval.md`.
- Evaluation: `tests/semantic_eval.lua` (see `docs/semantic-eval.md`). Search is linear in the number of chunks; see the scaling note there before expecting medium-vault latency.
