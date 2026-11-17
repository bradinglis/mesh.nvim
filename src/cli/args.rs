//! Argument parsing for the `mesh` CLI.
//!
//! The command surface is flat, so a small hand-rolled parser avoids pulling in
//! a dependency and keeps startup fast. Global and command options may be
//! interleaved; the first bare word is the command.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Table,
    Json,
    Jsonl,
}

#[derive(Debug, Clone)]
pub struct GlobalOpts {
    pub vault: Option<String>,
    pub db: Option<String>,
    pub format: Option<Format>,
    pub pretty: bool,
    pub limit: Option<usize>,
    pub all: bool,
    pub quiet: bool,
    pub color: bool,
    pub warm: bool,
}

impl Default for GlobalOpts {
    fn default() -> Self {
        Self {
            vault: None,
            db: None,
            format: None,
            pretty: false,
            limit: None,
            all: false,
            quiet: false,
            color: true,
            warm: false,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SearchFlags {
    pub context: Option<String>,
    pub group: Option<String>,
    pub graph_seeds: Option<bool>,
    pub refine_line: Option<bool>,
    pub max_per_note: Option<usize>,
    pub max_per_author: Option<usize>,
    pub high_score: Option<f32>,
    pub semantic: Option<f32>,
    pub keyword: Option<f32>,
    pub graph: Option<f32>,
}

#[derive(Debug)]
pub enum Command {
    Help,
    Version,
    Update,
    Embed { warm: bool },
    Notes,
    ThinNotes,
    Lines,
    Type(String),
    Tags,
    Tag(String),
    Anchors(String),
    Backlinks(String),
    Id(String),
    Search { prompt: String, flags: SearchFlags },
    Related { note_id: String, flags: SearchFlags },
}

#[derive(Debug)]
pub struct Invocation {
    pub globals: GlobalOpts,
    pub command: Command,
}

pub fn parse(argv: Vec<String>) -> Result<Invocation, String> {
    let mut globals = GlobalOpts::default();
    let mut search = SearchFlags::default();
    let mut positional: Vec<String> = Vec::new();
    let mut help = false;
    let mut version = false;

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].clone();
        if arg == "--" {
            positional.extend(argv[i + 1..].iter().cloned());
            break;
        }

        if let Some(rest) = arg.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (rest.to_string(), None),
            };
            let value = |i: &mut usize| take_value(&argv, i, &inline, &name);
            match name.as_str() {
                "vault" => globals.vault = Some(value(&mut i)?),
                "db" => globals.db = Some(value(&mut i)?),
                "format" => globals.format = Some(parse_format(&value(&mut i)?)?),
                "json" => globals.format = Some(Format::Json),
                "jsonl" => globals.format = Some(Format::Jsonl),
                "pretty" => globals.pretty = true,
                "limit" => globals.limit = Some(parse_usize(&value(&mut i)?, "--limit")?),
                "all" => globals.all = true,
                "quiet" => globals.quiet = true,
                "no-color" => globals.color = false,
                "warm" => globals.warm = true,
                "context" => search.context = Some(value(&mut i)?),
                "group" => search.group = Some(value(&mut i)?),
                "graph-seeds" => search.graph_seeds = Some(true),
                "no-graph-seeds" => search.graph_seeds = Some(false),
                "refine-line" => search.refine_line = Some(true),
                "no-refine-line" => search.refine_line = Some(false),
                "max-per-note" => {
                    search.max_per_note = Some(parse_usize(&value(&mut i)?, "--max-per-note")?)
                }
                "max-per-author" => {
                    search.max_per_author = Some(parse_usize(&value(&mut i)?, "--max-per-author")?)
                }
                "high-score" => {
                    search.high_score = Some(parse_f32(&value(&mut i)?, "--high-score")?)
                }
                "semantic" => search.semantic = Some(parse_f32(&value(&mut i)?, "--semantic")?),
                "keyword" => search.keyword = Some(parse_f32(&value(&mut i)?, "--keyword")?),
                "graph" => search.graph = Some(parse_f32(&value(&mut i)?, "--graph")?),
                "help" => help = true,
                "version" => version = true,
                other => return Err(format!("unknown option '--{other}'")),
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            match arg.as_str() {
                "-h" => help = true,
                "-V" => version = true,
                "-q" => globals.quiet = true,
                other => return Err(format!("unknown option '{other}'")),
            }
        } else {
            positional.push(arg);
        }
        i += 1;
    }

    if help {
        return Ok(Invocation {
            globals,
            command: Command::Help,
        });
    }
    if version {
        return Ok(Invocation {
            globals,
            command: Command::Version,
        });
    }

    let command = build_command(positional, &globals, search)?;
    Ok(Invocation { globals, command })
}

fn build_command(
    positional: Vec<String>,
    globals: &GlobalOpts,
    search: SearchFlags,
) -> Result<Command, String> {
    let mut iter = positional.into_iter();
    let Some(name) = iter.next() else {
        return Ok(Command::Help);
    };
    let rest: Vec<String> = iter.collect();
    let need = |rest: &[String]| -> Result<String, String> {
        rest.first()
            .cloned()
            .ok_or_else(|| format!("'{name}' needs an argument"))
    };

    match name.as_str() {
        "help" => Ok(Command::Help),
        "version" => Ok(Command::Version),
        "update" => Ok(Command::Update),
        "embed" => Ok(Command::Embed { warm: globals.warm }),
        "notes" => Ok(Command::Notes),
        "thin-notes" => Ok(Command::ThinNotes),
        "lines" => Ok(Command::Lines),
        "tags" => Ok(Command::Tags),
        "type" => Ok(Command::Type(need(&rest)?)),
        "tag" => Ok(Command::Tag(need(&rest)?)),
        "anchors" => Ok(Command::Anchors(need(&rest)?)),
        "backlinks" => Ok(Command::Backlinks(need(&rest)?)),
        "id" => {
            let title = rest.join(" ");
            if title.trim().is_empty() {
                return Err("'id' needs a title".to_string());
            }
            Ok(Command::Id(title))
        }
        "search" => {
            let prompt = rest.join(" ");
            if prompt.trim().is_empty() {
                return Err("'search' needs a prompt".to_string());
            }
            Ok(Command::Search {
                prompt,
                flags: search,
            })
        }
        "related" => Ok(Command::Related {
            note_id: need(&rest)?,
            flags: search,
        }),
        other => Err(format!("unknown command '{other}'")),
    }
}

fn take_value(
    argv: &[String],
    i: &mut usize,
    inline: &Option<String>,
    name: &str,
) -> Result<String, String> {
    if let Some(value) = inline {
        return Ok(value.clone());
    }
    *i += 1;
    argv.get(*i)
        .cloned()
        .ok_or_else(|| format!("option '--{name}' needs a value"))
}

fn parse_format(value: &str) -> Result<Format, String> {
    match value {
        "table" => Ok(Format::Table),
        "json" => Ok(Format::Json),
        "jsonl" => Ok(Format::Jsonl),
        other => Err(format!(
            "unknown format '{other}' (use table, json, or jsonl)"
        )),
    }
}

fn parse_usize(value: &str, name: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{name} expects a non-negative integer, got '{value}'"))
}

fn parse_f32(value: &str, name: &str) -> Result<f32, String> {
    value
        .parse()
        .map_err(|_| format!("{name} expects a number, got '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_global_flags_anywhere() {
        let inv = parse(args(&[
            "--vault",
            "/v",
            "notes",
            "--format=json",
            "--limit",
            "5",
        ]))
        .unwrap();
        assert_eq!(inv.globals.vault.as_deref(), Some("/v"));
        assert_eq!(inv.globals.format, Some(Format::Json));
        assert_eq!(inv.globals.limit, Some(5));
        assert!(matches!(inv.command, Command::Notes));
    }

    #[test]
    fn joins_search_prompt_words() {
        let inv = parse(args(&["search", "pasta", "with", "garlic", "--limit", "3"])).unwrap();
        match inv.command {
            Command::Search { prompt, .. } => assert_eq!(prompt, "pasta with garlic"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_search_options() {
        let inv = parse(args(&[
            "related",
            "Marx",
            "--group",
            "anchor",
            "--max-per-author",
            "1",
            "--high-score",
            "0.9",
            "--semantic",
            "0.7",
        ]))
        .unwrap();
        match inv.command {
            Command::Related { note_id, flags } => {
                assert_eq!(note_id, "Marx");
                assert_eq!(flags.group.as_deref(), Some("anchor"));
                assert_eq!(flags.max_per_author, Some(1));
                assert_eq!(flags.high_score, Some(0.9));
                assert_eq!(flags.semantic, Some(0.7));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(parse(args(&["--nope"])).is_err());
        assert!(parse(args(&["notes", "--limit", "x"])).is_err());
    }
}
