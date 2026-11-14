# mesh.nvim

Neovim plugin for a Markdown note vault: Rust `cdylib` (`mesh`) indexes notes into DuckDB; Lua under `lua/mesh-nvim/` provides UI, pickers, and completion. Depends on `snacks.nvim` and `blink.cmp`.

## Build & dev

- Toolchain is pinned to nightly Rust through the flake: `nix develop` (or rely on `.envrc` + direnv). The crate no longer uses unstable feature gates, but use the flake's pinned toolchain for reproducible builds.
- DuckDB is linked dynamically (crate feature `bundled` is disabled). `libduckdb` must be available at build and runtime; the flake provides it.
- Lua loads the library from `target/release/libmesh.so` (`util.get_paths().release_dir`). After Rust edits run `cargo build --release`, not a debug build.
- Build plugin via Nix: `nix build` (copies `libmesh.so` into the vim plugin tree). Nix builds run `cargo test --lib` too, so `tests/` must stay tracked in git (flakes ignore untracked files) and listed in the `src` fileset in `flake.nix`.
- Quick compile check: `cargo check`.

## Testing

- `cargo test --lib` runs 15 tests across core modules. Run annotation tests with `cargo test --lib highlights::tests::test_one`.
- PDF fixtures live in `tests/*.pdf`; keep them present when running annotation tests.
- Last green baseline before the annotations refactor: `a6746c5` (`origin/main`).
- Last commit before this cleanup refactor: `6c2d2c7`.

## Architecture

- Rust entry: `src/lib.rs` exports the `mesh` Lua module; every function takes `(notes_dir, db_file, ...)`.
- Lua entry: `lua/mesh-nvim/init.lua` (`setup(opts)`). `lua/mesh-nvim/binary.lua` loads `mesh` from `target/release`.
- DB: one DuckDB file per vault at `stdpath("data")/mesh/<vault>.duckdb`; schema in `sql/clear.sql`; read queries in `sql/notes_details.sql`.
- Notes: recursively scans the vault for `.md`, skipping `.git` and `templates`. Hashing uses `rapidhash`.
- Vault config: `vaults` table plus a `default` key that may be a vault name (string) or a vault table. The shipped default (`vaults.default = "none"`) is a placeholder and must be overridden.
- Stronger types: `NoteType`, `RefKind` (`intext_link`, `frontmatter_reference`, `author`, `parent`), and `Location` (`frontmatter`/`body`) replace stringly typed fields in the Rust core.

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
