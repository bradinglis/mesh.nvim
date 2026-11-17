# `mesh` CLI

The `mesh` binary exposes the same operations as the Neovim library (they share
`src/api.rs`) so scripts and agents can query a vault without starting Neovim.

## Building and running

```sh
nix build .#mesh-cli          # ./result/bin/mesh
nix run .#mesh -- notes       # or: nix run .#mesh -- search "commodity fetishism"
```

Outside Nix, build with `cargo build --release` and set `ORT_DYLIB_PATH` before
using the semantic commands. Non-semantic commands work without it.

## Vault and database

The vault directory comes from `--vault` or `MESH_VAULT`; the database from
`--db` or `MESH_DB`. When only the vault is given, the database defaults to
`$XDG_DATA_HOME/mesh/<vault>.duckdb` (the same name Neovim derives).

Read commands open the database **read-only**: a database that belongs to a
different vault is reported instead of being cleared.

```sh
mesh update  --vault ~/zettel
mesh notes   --vault ~/zettel
mesh search  "how does commodity fetishism work" --vault ~/zettel
```

## Commands

| Command | Description |
| --- | --- |
| `update` | Scan the vault and update the index (read-write) |
| `embed [--warm]` | Refresh embeddings; `--warm` only loads the model |
| `notes` | List notes |
| `thin-notes` | List notes without tags or authors |
| `lines` | List note body lines |
| `type <type>` | List notes of a type |
| `tags` | List tags with counts |
| `tag <tag>` | List notes carrying a tag |
| `anchors <note-id>` | List a note's headings |
| `backlinks <note-id>` | List links to and from a note |
| `id <title>` | Print the note id for a title (pure) |
| `search <prompt>` | Semantic prompt search |
| `related <note-id>` | Notes related to a note |
| `help`, `version` | Usage and version |

`search` and `id` join their remaining arguments, so quoting is optional:
`mesh search pasta with garlic`.

## Output

Tables are the default on a terminal, JSON when stdout is a pipe. Choose
explicitly with `--format table|json|jsonl`, `--json`, or `--jsonl`. Data goes
to stdout; progress and errors go to stderr.

```sh
mesh notes --format json | jq '.[].id'
mesh thin-notes --jsonl | while read -r line; do echo "$line"; done
mesh search "gift exchange" --pretty --limit 5
```

- `--limit <n>` caps both the rows printed and the number of search results
  (default 100).
- `--all` prints every row.
- `--pretty` indents JSON.
- `--quiet` suppresses status messages from `update`/`embed`.
- Lists in JSON drop `null` entries, matching the Lua module.

Table columns are a curated subset per command; use `--format json` for the full
record (paths, scores, and the individual signal weights).

### Search options

| Option | Meaning |
| --- | --- |
| `--context <note-id>` | Blend the query with a note's embedding |
| `--group <mixed\|anchor\|note>` | Restrict the result shape |
| `--max-per-note <n>` | Max results per note (0 disables; default 2) |
| `--max-per-author <n>` | Max child sources per author (0 disables; default 2) |
| `--high-score <f>` | Score that bypasses the per-author cap (default 0.85) |
| `--[no-]graph-seeds` | Use graph seeds for prompt search |
| `--[no-]refine-line` | Refine hits to the best matching line |
| `--semantic <f>` `--keyword <f>` `--graph <f>` | Override signal weights |

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Runtime error (missing database, vault mismatch, …) |
| `2` | Usage error (unknown command or option, bad value) |

## Testing

`cargo test --test cli` runs the integration tests (`tests/cli.rs`), which build
a throwaway vault and exercise the read commands without a model or network.
