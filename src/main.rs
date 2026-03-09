use std::collections::HashMap;
use std::fs::{self, FileType};
use std::io::{self, BufRead, Lines};
use std::path::Path;
use std::sync::LazyLock;
use thiserror::Error;
use regex::{Regex, RegexSet};

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("A custom error occurred")]
    OtherError,
}

#[derive(Debug)]
enum NotesPath {
    File(Note),
    Dir(Vec<NotesPath>),
}

#[derive(Clone, Debug)]
struct Note {
    frontmatter: FrontMatter,
    contents: Vec<String>,
}

pub type Result<T> = std::result::Result<T, MyError>;
type LinesIter = std::io::Lines<std::io::BufReader<std::fs::File>>;

// #[derive(Debug)]
enum ParseState {
    NoFrontMatter(),
    InFrontMatter(FrontMatterBuild),
    CompletedFrontMatter(Note),
}

// #[derive(Debug)]
struct FrontMatterBuild {
    id: Option<String>,
    aliases: Option<String>,
    note_type: Option<String>,
    tags: Option<Vec<String>>,
    references: Option<Vec<String>>,
    source_parents: Option<Vec<String>>,
    author: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
struct FrontMatter {
    id: String,
    aliases: Option<String>,
    note_type: NoteType,
    tags: Vec<String>,
    references: Vec<String>,
    source_parents: Vec<String>,
    author: Vec<String>,
}

#[derive(Clone, Debug)]
enum NoteType {
    Source,
    Author,
    Note,
}

fn main() -> Result<()> {
    // Read the current directory (".").
    // let main_dir = Path::new("../testing/zettel");
    // let test = read_dir(main_dir)?;
    // print!("{:#?}", test);

    // let main_dir = Path::new("../testing/zettel");
    // read_file(Path::new("../testing/zettel/notes/social-form-things.md"))

    parse_from_line();
    Ok(())
}

pub enum FoldState<T> {
    Continue(T),
    Break(T),
}

fn read_file(file: &Path) -> Result<Note> {
    println!("File: {}", file.display());

    if file.extension().and_then(|x| x.to_str()).unwrap_or("") != "md" {
        return Err(MyError::OtherError);
    }

    let file = fs::File::open(file)?;
    let reader = io::BufReader::new(file);

    // let x = reader.lines();
    let (parsed, rest) = reader
        .lines()
        .fold_rest(ParseState::NoFrontMatter, |state, x| {
            let x = x?;

            match state {
                ParseState::CompletedFrontMatter(note) => {
                    Ok(FoldState::Break(ParseState::CompletedFrontMatter(note)))
                }
                _ => Ok(FoldState::Continue(parse_frontmatter(state, x)?)),
            }
        })?;

    let mut note = match parsed {
        ParseState::CompletedFrontMatter(note) => Ok(note),
        _ => Err(MyError::OtherError),
    }?;

    note.contents = rest.map(|x| x.unwrap_or("".to_string())).collect();

    Ok(note)
}

fn parse_from_line() -> Result<()> {
    let tag_regex = Regex::new(r"(#[a-zA-Z\-]*)").unwrap();
    let text = "- A particular vulgarisation — *productive force determinism* ([Social Ontology of Economic Power][1]) — takes for granted that social development is determined by the (purely independent) development of the *productive forces*. ";

    let tags = content_get_reference_links(text.to_string());
    print!("{:#?}", tags);

    Ok(())
}

fn content_get_tags(line: String) -> Vec<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(#[a-zA-Z\-]*)").unwrap());

    let mut tags = vec![];
    for (_, [tag]) in RE.captures_iter(line.as_str()).map(|c| c.extract()) {
        tags.push(tag.to_string());
    }
    tags
}

fn content_get_links(line: String) -> Vec<(String, String)> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\|\]]+)\|?([^\|\]]*)\]\]").unwrap());

    let mut links = vec![];
    for (_, [id, title]) in RE.captures_iter(line.as_str()).map(|c| c.extract()) {
        links.push((id.to_string(),title.to_string()));
    }
    links
}

fn content_get_reference_links(line: String) -> Vec<(String, String)> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([0-9]+)\]").unwrap());

    let mut links = vec![];
    for (_, [id, title]) in RE.captures_iter(line.as_str()).map(|c| c.extract()) {
        links.push((id.to_string(),title.to_string()));
    }
    links
}

fn read_dir(dir: &Path) -> Result<Vec<NotesPath>> {
    let dir_name = dir.file_name().and_then(|x| x.to_str()).unwrap_or("");
    if dir_name == ".git" || dir_name == "templates" {
        return Err(MyError::OtherError);
    }

    let something: Vec<NotesPath> = fs::read_dir(dir)?
        .map(|entry| {
            let entry = entry?;
            read_path(entry.path().as_path())
        })
        .filter_map(|x| x.ok())
        .collect();

    Ok(something)
}

fn read_path(path: &Path) -> Result<NotesPath> {
    println!("{:#?}", path);

    match (path.is_dir(), path.is_file()) {
        (true, _) => Ok(NotesPath::Dir(read_dir(path)?)),
        (_, true) => Ok(NotesPath::File(read_file(path)?)),
        (_, _) => Err(MyError::OtherError),
    }
    // Ok(())
}

fn parse_frontmatter(state: ParseState, line: String) -> Result<ParseState> {
    let frontmatter_boundary = line == "---";

    match (state, frontmatter_boundary) {
        (ParseState::NoFrontMatter, true) => Ok(init_frontmatter_parse()),
        (ParseState::InFrontMatter(front_matter_build), false) => Ok(ParseState::InFrontMatter(
            add_line_to_frontmatter(line, front_matter_build),
        )),
        (ParseState::InFrontMatter(front_matter_build), true) => {
            format_frontmatter(front_matter_build)
        }
        _ => Err(MyError::OtherError),
    }
}

fn init_frontmatter_parse() -> ParseState {
    ParseState::InFrontMatter(FrontMatterBuild {
        id: None,
        aliases: None,
        note_type: None,
        tags: None,
        references: None,
        source_parents: None,
        author: None,
    })
}

fn add_line_to_frontmatter(line: String, front_matter_build: FrontMatterBuild) -> FrontMatterBuild {
    match line.split_once(": ") {
        Some(("id", suff)) => FrontMatterBuild {
            id: Some(suff.to_string()),
            ..front_matter_build
        },
        Some(("aliases", suff)) => FrontMatterBuild {
            aliases: Some(suff.to_string()),
            ..front_matter_build
        },
        Some(("type", suff)) => FrontMatterBuild {
            note_type: Some(suff.to_string()),
            ..front_matter_build
        },
        Some(("source-parents", suff)) => FrontMatterBuild {
            source_parents: Some(suff.split_whitespace().map(|x| x.to_string()).collect()),
            ..front_matter_build
        },
        Some(("author", suff)) => FrontMatterBuild {
            author: Some(suff.split_whitespace().map(|x| x.to_string()).collect()),
            ..front_matter_build
        },
        Some(("references", suff)) => FrontMatterBuild {
            references: Some(suff.split_whitespace().map(|x| x.to_string()).collect()),
            ..front_matter_build
        },
        Some(("tags", suff)) => FrontMatterBuild {
            tags: Some(suff.split_whitespace().map(|x| x.to_string()).collect()),
            ..front_matter_build
        },
        _ => panic!(),
    }
}

fn format_frontmatter(front_matter_build: FrontMatterBuild) -> Result<ParseState> {
    let note_type = front_matter_build.note_type.ok_or(MyError::OtherError)?;

    Ok(ParseState::CompletedFrontMatter(Note {
        frontmatter: FrontMatter {
            id: front_matter_build.id.ok_or(MyError::OtherError)?,
            aliases: front_matter_build.aliases,
            note_type: note_type_from_string(note_type)?,
            tags: front_matter_build.tags.unwrap_or(vec![]),
            references: front_matter_build.references.unwrap_or(vec![]),
            source_parents: front_matter_build.source_parents.unwrap_or(vec![]),
            author: front_matter_build.author.unwrap_or(vec![]),
        },
        contents: vec![],
    }))
}

fn note_type_from_string(s: String) -> Result<NoteType> {
    match s.as_str() {
        "author" => Ok(NoteType::Author),
        "source" => Ok(NoteType::Source),
        "note" => Ok(NoteType::Note),
        _ => Err(MyError::OtherError),
    }
}

pub trait FoldRest {
    type Item;

    fn fold_rest<C, F>(self, init: C, f: F) -> Result<(C, Self)>
    where
        Self: Sized,
        F: FnMut(C, Self::Item) -> Result<FoldState<C>>;
}

impl<B: BufRead> FoldRest for Lines<B> {
    type Item = std::io::Result<String>;

    fn fold_rest<C, F>(mut self, init: C, mut f: F) -> Result<(C, Self)>
    where
        Self: Iterator<Item = std::io::Result<String>> + Sized,
        F: FnMut(C, Self::Item) -> Result<FoldState<C>>,
    {
        let mut accum = init;
        while let Some(x) = self.next() {
            let t = f(accum, x)?;
            match t {
                FoldState::Continue(cont) => accum = cont,
                FoldState::Break(b) => {
                    accum = b;
                    break;
                }
            }
        }
        Ok((accum, self))
    }
}
