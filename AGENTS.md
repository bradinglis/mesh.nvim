# AGENTS.md

Guidance for AI agents and humans working in this repository.

## What this is

`mesh` is a small Rust crate that reads **highlight annotations** from a PDF and
renders the highlighted text as **Markdown** using
[`pdf_oxide`](https://crates.io/crates/pdf_oxide).

It is the extraction core of the former `mesh-nvim` project (the flake still
uses the package name `mesh-nvim-lib` and references obsolete `lua`/`sql`
paths). This repository is the standalone Rust experiment; the editor/MCP
integration is not present here.

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Thin demo binary. Hardcodes `tests/test_4.pdf` and prints the extracted `Vec<String>` with `{:#?}`. |
| `src/annotations.rs` | Core logic: `get_annotations()` and four `#[cfg(test)]` tests. |
| `tests/*.pdf` | Binary PDF fixtures (**required** by the tests). |
| `Cargo.toml` / `Cargo.lock` | Crate manifest, `pdf_oxide = 0.3.78`. |
| `flake.nix` / `flake.lock` / `.envrc` / `.direnv` | Nix dev shell and package build (nightly toolchain). |

There is no `lib.rs`, CI config, `Makefile`, or `justfile`.

## Toolchain

- **Nightly Rust is required.** `src/main.rs` enables unstable features
  (`#![feature(try_trait_v2)]`, `#![feature(iter_intersperse)]`), so stable
  toolchains will not compile the crate.
- Edition 2024.
- There is no `rust-toolchain.toml`; the flake selects the latest nightly
  (verified with `1.101.0-nightly`).

## Commands

```sh
cargo check        # fast type check
cargo build
cargo run          # runs the hardcoded fixture in src/main.rs
cargo test         # all four tests pass

nix develop        # or `direnv allow` to enter the dev shell
```

⚠️ **`nix build` is currently broken.** `flake.nix` builds `src` from a
`fileset` that still includes `./sql` and `./lua`, which no longer exist in
this repo. Remove those entries before relying on the Nix package.

## How `get_annotations` works

`get_annotations(path) -> Result<Vec<String>, pdf_oxide::Error>`:

1. Open the PDF with `PdfDocument::open`.
2. Iterate `doc.page_indices()` and call `doc.get_annotations(page)`.
3. Keep only `AnnotationSubtype::Highlight` annotations.
4. For each annotation, turn every `quad_points` entry into an axis-aligned
   `Rect` (the producer may list the corners in either winding order).
5. Extract characters with `doc.extract_chars(page)` and crop them per quad
   by glyph centre. This is more precise than span/rect extraction for
   highlights that start or stop inside a glyph.
6. Drop a leading fragment when the quad begins mid-word, synthesise spaces
   for positioning gaps, and join lines in top-to-bottom order.
7. Undo end-of-line hyphenation and emit `*emphasis*` for italic runs (real
   italic fonts plus font changes within body-font lines).
8. Decode OCR entities such as `&dquo;` and return one `String` per
   annotation.

## Current state / known issues

- All four tests pass (`test_one`, `test_two`, `test_three`, `test_four`).
- `test_three` has real Times-Roman/Times-Italic fonts and exercises source
  emphasis.
- `test_two` and `test_four` are PDFium/scanned re-renders without usable
  italic font data. `test_four` is a CCITT scan plus an invisible OCR layer;
  its expectations therefore assert the OCR layer's text (`graspea`, no
  closing quote, etc.), not the visually corrected article.
- `cargo check` still reports 3 pre-existing warnings in `src/main.rs`:
  unused `try_trait_v2` and `iter_intersperse` features and an unused
  `get_annotations` import.
- `nix build` is still broken (`./sql` and `./lua` in the flake fileset).

## Conventions and gotchas

- **Do not modify, regenerate, or move the PDFs in `tests/`** — they are
  binary fixtures asserted against by exact string comparisons.
- Tests are inline in `src/annotations.rs` (`#[cfg(test)]`), not under
  `tests/`, because `tests/` holds the PDFs.
- Keep changes consistent with the existing minimal, dependency-light style
  (only `pdf_oxide`, `log`, `env_logger`).
