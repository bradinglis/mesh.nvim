//! The `mesh` command line interface.
//!
//! It exposes the same operations as the Neovim library (through [`crate::api`])
//! for scripts and agents. Output is a rendered table on a terminal and JSON
//! elsewhere, with `--format`/`--json`/`--jsonl` to choose explicitly.

mod args;
mod json;
mod render;

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::api;
use crate::db::{Db, DbValue};
use crate::semantic_search::{Group, SearchOpts};

use args::{Command, Format, GlobalOpts, Invocation, SearchFlags};

const DEFAULT_ROWS: usize = 100;
const MAX_COL: usize = 60;

const NOTE_COLUMNS: &[&str] = &["id", "type", "title", "tags", "authors", "path"];
const SEARCH_COLUMNS: &[&str] = &[
    "score", "note_id", "heading", "line", "title", "tags", "snippet",
];

const USAGE: &str = "\
mesh - query a mesh.nvim vault

USAGE:
    mesh [OPTIONS] <COMMAND> [ARGS]

COMMANDS:
    update                     Index the vault (scan and update the database)
    embed [--warm]             Refresh embeddings (--warm only loads the model)
    notes                      List notes
    thin-notes                 List notes without tags or authors
    lines                      List note body lines
    type <type>                List notes of a type
    tags                       List tags
    tag <tag>                  List notes carrying a tag
    anchors <note-id>          List a note's headings
    backlinks <note-id>        List links to and from a note
    id <title>                 Print the note id for a title
    search <prompt>            Semantic prompt search
    related <note-id>          Notes related to a note
    help                       Show this help
    version                    Show the version

OPTIONS:
    --vault <dir>              Vault directory (env MESH_VAULT)
    --db <file>                Database file (env MESH_DB); defaults to
                               $XDG_DATA_HOME/mesh/<vault>.duckdb
    --format <table|json|jsonl>  Output format (table on a TTY, json otherwise)
    --json, --jsonl            Shorthands for --format json / jsonl
    --pretty                   Pretty-print JSON
    --limit <n>                Max rows and search results (default 100)
    --all                      Do not cap the number of rows
    --no-color                 Disable ANSI colour (env NO_COLOR)
    -q, --quiet                Suppress status messages
    -h, --help                 Show this help
    -V, --version              Show the version

SEARCH OPTIONS:
    --context <note-id>        Blend the query with a note's embedding
    --group <mixed|anchor|note>
    --max-per-note <n>         Max results per note (0 disables)
    --max-per-author <n>       Max child sources per author (0 disables)
    --high-score <f>           Score that bypasses the per-author cap
    --[no-]graph-seeds         Use graph seeds for prompt search
    --[no-]refine-line         Refine hits to the best matching line
    --semantic <f> --keyword <f> --graph <f>   Override signal weights
";

pub fn run(argv: Vec<String>) -> i32 {
    let invocation = match args::parse(argv) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("mesh: {message}");
            eprintln!("Try 'mesh help' for usage.");
            return 2;
        }
    };

    match invocation.command {
        Command::Help => {
            print!("{USAGE}");
            0
        }
        Command::Version => {
            println!("mesh {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => match execute(&invocation) {
            Ok(()) => 0,
            Err(message) => {
                eprintln!("mesh: {message}");
                1
            }
        },
    }
}

fn execute(inv: &Invocation) -> Result<(), String> {
    let g = &inv.globals;
    let format = g.format.unwrap_or_else(default_format);
    let color = g.color && format == Format::Table && std::io::stdout().is_terminal();

    match &inv.command {
        Command::Update => {
            let (vault, db) = paths_for_write(g)?;
            let log = api::update(&vault, &db).map_err(stringify)?;
            emit_message(&log, format, g);
        }
        Command::Embed { warm } => {
            if *warm {
                let db = resolve_db(g, vault_from(g).as_deref())?;
                let message = api::warm_model(&db).map_err(stringify)?;
                emit_message(&message, format, g);
            } else {
                let (vault, db) = paths_for_write(g)?;
                let message = run_embed(&vault, &db, g)?;
                emit_message(&message, format, g);
            }
        }
        Command::Id(title) => {
            emit_data(DbValue::String(api::id_gen(title)), format, g, color, &[]);
        }
        Command::Notes => {
            let value = with_read(g, |db, _| api::notes(db))?;
            emit_data(value, format, g, color, NOTE_COLUMNS);
        }
        Command::ThinNotes => {
            let value = with_read(g, |db, _| api::thin_notes(db))?;
            emit_data(value, format, g, color, &["id", "type", "title", "path"]);
        }
        Command::Lines => {
            let value = with_read(g, |db, _| api::lines(db))?;
            emit_data(value, format, g, color, &["line", "text", "id", "title"]);
        }
        Command::Type(note_type) => {
            let value = with_read(g, |db, _| api::type_notes(db, note_type))?;
            emit_data(value, format, g, color, NOTE_COLUMNS);
        }
        Command::Tags => {
            let value = with_read(g, |db, _| api::tags(db))?;
            emit_data(value, format, g, color, &["tag", "count"]);
        }
        Command::Tag(tag) => {
            let value = with_read(g, |db, _| api::tag_notes(db, tag))?;
            emit_data(value, format, g, color, &["id", "type", "title", "line"]);
        }
        Command::Anchors(id) => {
            let value = with_read(g, |db, _| api::note_anchors(db, id))?;
            emit_data(value, format, g, color, &["id", "title"]);
        }
        Command::Backlinks(id) => {
            let value = with_read(g, |db, _| api::backlinks(db, id))?;
            emit_data(
                value,
                format,
                g,
                color,
                &["id", "type", "title", "direction", "kind", "line"],
            );
        }
        Command::Search { prompt, flags } => {
            let opts = search_opts(g, flags)?;
            let value = with_read(g, |db, path| {
                api::search_prompt(db, path, prompt, flags.context.clone(), &opts)
            })?;
            emit_data(value, format, g, color, SEARCH_COLUMNS);
        }
        Command::Related { note_id, flags } => {
            let opts = search_opts(g, flags)?;
            let value = with_read(g, |db, path| api::search_note(db, path, note_id, &opts))?;
            emit_data(value, format, g, color, SEARCH_COLUMNS);
        }
        Command::Help | Command::Version => unreachable!("handled in run()"),
    }

    Ok(())
}

fn search_opts(g: &GlobalOpts, flags: &SearchFlags) -> Result<SearchOpts, String> {
    let mut opts = SearchOpts::default();
    if let Some(limit) = g.limit {
        opts.limit = limit;
    }
    if let Some(group) = &flags.group {
        opts.group = Group::parse(group)
            .ok_or_else(|| format!("unknown group '{group}' (use mixed, anchor, or note)"))?;
    }
    if let Some(value) = flags.graph_seeds {
        opts.graph_seeds = value;
    }
    if let Some(value) = flags.refine_line {
        opts.refine_line = value;
    }
    if let Some(value) = flags.max_per_note {
        opts.max_per_note = value;
    }
    if let Some(value) = flags.max_per_author {
        opts.max_per_author = value;
    }
    if let Some(value) = flags.high_score {
        opts.high_score = value;
    }
    opts.weights.semantic = flags.semantic;
    opts.weights.keyword = flags.keyword;
    opts.weights.graph = flags.graph;
    Ok(opts)
}

fn run_embed(vault: &str, db: &str, g: &GlobalOpts) -> Result<String, String> {
    let mut printed = false;
    let mut last = usize::MAX;
    let mut progress = |done: usize, total: usize| {
        if g.quiet || total == 0 {
            return;
        }
        let pct = done * 100 / total;
        if pct == last {
            return;
        }
        last = pct;
        printed = true;
        eprint!("\rembedding {done}/{total} ({pct}%)");
        let _ = std::io::stderr().flush();
    };
    let result = api::embed_refresh(vault, db, &mut progress);
    if printed {
        eprintln!();
    }
    result.map_err(stringify)
}

fn with_read<T>(
    g: &GlobalOpts,
    f: impl FnOnce(&Db, &str) -> crate::Result<T>,
) -> Result<T, String> {
    let vault = vault_from(g);
    let db_path = resolve_db(g, vault.as_deref())?;
    if !Path::new(&db_path).exists() {
        return Err(format!(
            "no database at {db_path}; run `mesh update` first or pass --db"
        ));
    }
    let expected = vault.as_deref().map(Path::new);
    let db = Db::open_read_only(Path::new(&db_path), expected).map_err(stringify)?;
    f(&db, &db_path).map_err(stringify)
}

fn paths_for_write(g: &GlobalOpts) -> Result<(String, String), String> {
    let vault = vault_from(g).ok_or("specify --vault (or set MESH_VAULT)")?;
    let db = resolve_db(g, Some(&vault))?;
    Ok((vault, db))
}

fn vault_from(g: &GlobalOpts) -> Option<String> {
    g.vault
        .clone()
        .or_else(|| std::env::var("MESH_VAULT").ok().filter(|s| !s.is_empty()))
}

fn resolve_db(g: &GlobalOpts, vault: Option<&str>) -> Result<String, String> {
    if let Some(db) = &g.db {
        return Ok(db.clone());
    }
    if let Some(db) = std::env::var("MESH_DB").ok().filter(|s| !s.is_empty()) {
        return Ok(db);
    }
    let vault = vault.ok_or("specify --db or --vault (or set MESH_DB or MESH_VAULT)")?;
    let name = Path::new(vault)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("mesh");
    Ok(data_home()?
        .join("mesh")
        .join(format!("{name}.duckdb"))
        .to_string_lossy()
        .into_owned())
}

fn data_home() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set; pass --db explicitly")?;
    Ok(PathBuf::from(home).join(".local/share"))
}

fn emit_data(value: DbValue, format: Format, g: &GlobalOpts, color: bool, preferred: &[&str]) {
    match format {
        Format::Table => {
            let max_rows = if g.all {
                None
            } else {
                Some(g.limit.unwrap_or(DEFAULT_ROWS))
            };
            let opts = render::TableOptions {
                preferred,
                max_rows,
                max_col: MAX_COL,
                color,
            };
            println!("{}", render::render(&value, &opts));
        }
        Format::Json => println!("{}", json::to_string(&value, g.pretty)),
        Format::Jsonl => println!("{}", json::to_jsonl(&value)),
    }
}

fn emit_message(message: &str, format: Format, g: &GlobalOpts) {
    if g.quiet {
        return;
    }
    match format {
        Format::Table => println!("{message}"),
        Format::Json | Format::Jsonl => {
            let value = DbValue::Table(vec![(
                "message".to_string(),
                DbValue::String(message.to_string()),
            )]);
            println!(
                "{}",
                json::to_string(&value, g.pretty && format == Format::Json)
            );
        }
    }
}

fn default_format() -> Format {
    if std::io::stdout().is_terminal() {
        Format::Table
    } else {
        Format::Json
    }
}

fn stringify(error: crate::MyError) -> String {
    match error {
        crate::MyError::String(message) => message,
        other => other.to_string(),
    }
}
