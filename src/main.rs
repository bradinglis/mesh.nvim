use regex::{Regex};
use std::fs::{self};
use std::io::{self, BufRead, Lines};
use std::path::Path;
use std::sync::LazyLock;
use thiserror::Error;

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
    id: String,
    aliases: Option<String>,
    note_type: NoteType,
    tags: Vec<Ref>,
    references: Vec<Ref>,
    body_tags: Vec<Ref>,
    body_links: Vec<Ref>,
    contents: Vec<ContentLine>,
    source_parents: Vec<String>,
    author: Vec<String>,
    lines: Option<usize>,
    frontmatter_lines: Option<usize>,
}

#[derive(Clone, Debug)]
struct Ref {
    from: RefNode,
    to: RefNode,
    loc: RefLoc,
    alias: Option<String>,
}

#[derive(Clone, Debug)]
enum RefNode {
    Note(String),
    Tag(String),
    Local(usize),
}

#[derive(Clone, Debug)]
enum RefLoc {
    FrontMatter,
    Body(usize),
}

pub type Result<T> = std::result::Result<T, MyError>;

// #[derive(Debug)]
enum ParseState {
    Nothing,
    InYaml(FrontMatterBuild),
    InMd(Note),
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
struct ContentLine {
    text: String,
    line_num: usize,
    note: String,
}

#[derive(Clone, Debug)]
enum NoteType {
    Source,
    Author,
    Note,
}

fn main() -> Result<()> {
    // Read the current directory (".").
    // let main_dir = Path::new("../zettel/");
    // let test = read_dir(main_dir)?;
    // print!("{:#?}", test);

    // let main_dir = Path::new("../testing/zettel");
    let x = read_file(Path::new("../testing/zettel/sources/s_neel_arms-hand.md"));
    print!("{:#?}", x);

    // parse_from_line();
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

    let ((parsed, frontmatter_line_num), rest): ((ParseState, usize), _) = reader
        .lines()
        .fold_rest((ParseState::Nothing, 0), |(state, line_num), x| {
            let line_num = line_num + 1;
            let x = x?;
            match state {
                ParseState::InMd(note) => Ok(FoldState::Break((ParseState::InMd(note), line_num))),
                _ => Ok(FoldState::Continue((
                    parse_frontmatter(state, x)?,
                    line_num,
                ))),
            }
        })?;

    let mut note = match parsed {
        ParseState::InMd(note) => Ok(note),
        _ => Err(MyError::OtherError),
    }?;

    note.frontmatter_lines = Some(frontmatter_line_num);
    let id = note.id.clone();


    let (contents, body_tags, body_links, refs, ref_list, line_num) = rest
        .fold((vec![], vec![], vec![], vec![], vec![], 0), |x, y| {
            parse_body(x, y, &id)
        });

    let mut ref_list = ref_list;

    ref_list.sort_by_key(|x| match x.from {
        RefNode::Note(_) => 999,
        RefNode::Tag(_) => 999,
        RefNode::Local(x) => x,
    });

    let refs = refs
        .iter()
        .map(|x| Ref {
            from: x.from.clone(),
            to: {
                match x.to.clone() {
                    RefNode::Local(y) => ref_list[y].to.clone(),
                    z => z,
                }
            },
            loc: x.loc.clone(),
            alias: x.alias.clone(),
        })
        .collect();

    note.contents = contents;
    note.body_tags = body_tags;
    note.body_links = app(body_links, refs);
    note.lines = Some(line_num);

    Ok(note)
}

fn parse_body(
    (contents, body_tags, body_links, refs, ref_list, line_num): (
        Vec<ContentLine>,
        Vec<Ref>,
        Vec<Ref>,
        Vec<Ref>,
        Vec<Ref>,
        usize,
    ),
    z: std::result::Result<String, io::Error>,
    note_id: &str,
) -> (
    Vec<ContentLine>,
    Vec<Ref>,
    Vec<Ref>,
    Vec<Ref>,
    Vec<Ref>,
    usize,
) {
    let text = z.unwrap_or("".to_string());
    let line_num = line_num + 1;

    let (tags, links, n_refs, n_ref_list) = parse_from_line(&text, note_id, &line_num);

    let body_tags = app(body_tags, tags);
    let body_links = app(body_links, links);
    let refs = app(refs, n_refs);
    let ref_list = app(ref_list, n_ref_list);

    let contents = push(
        contents,
        ContentLine {
            note: note_id.to_string(),
            text,
            line_num,
        },
    );

    (contents, body_tags, body_links, refs, ref_list, line_num)
}

fn app<T>(mut first: Vec<T>, mut second: Vec<T>) -> Vec<T> {
    first.append(&mut second);
    first
}

fn push<T>(mut first: Vec<T>, second: T) -> Vec<T> {
    first.push(second);
    first
}

fn parse_from_line(line: &str, id: &str, lnum: &usize) -> (Vec<Ref>, Vec<Ref>, Vec<Ref>, Vec<Ref>) {
    let tags = content_get_tags(line, id, lnum);
    let links = content_get_links(line, id, lnum);
    let refs_intext = content_get_reference_intext(line, id, lnum);
    let refs_list = content_get_reference_list(line, id, lnum);
    (tags, links, refs_intext, refs_list)
}

fn content_get_tags(line: &str, id: &str, lnum: &usize) -> Vec<Ref> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#([a-zA-Z\-]+)").unwrap());

    let mut tags = vec![];
    for (_, [tag]) in RE.captures_iter(line).map(|c| c.extract()) {
        tags.push(Ref {
            from: RefNode::Note(id.to_string()),
            to: RefNode::Tag(tag.to_string()),
            loc: RefLoc::Body(*lnum),
            alias: None,
        });
    }
    tags
}

fn content_get_links(line: &str, id: &str, lnum: &usize) -> Vec<Ref> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\[\[([^\|\]]+)\|?([^\|\]]*)\]\]").unwrap());

    let mut links = vec![];
    for (_, [to_id, title]) in RE.captures_iter(line).map(|c| c.extract()) {
        links.push(Ref {
            from: RefNode::Note(id.to_string()),
            to: RefNode::Note(to_id.to_string()),
            loc: RefLoc::Body(*lnum),
            alias: Some(title.to_string()),
        });
    }
    links
}

fn content_get_reference_intext(line: &str, id: &str, lnum: &usize) -> Vec<Ref> {
    static RE_INTEXT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([0-9]+)\]").unwrap());
    let mut in_text_links = vec![];
    for (_, [name, ref_num]) in RE_INTEXT.captures_iter(line).map(|c| c.extract()) {
        in_text_links.push(Ref {
            from: RefNode::Note(id.to_string()),
            to: RefNode::Local(ref_num.parse().unwrap_or(0)),
            loc: RefLoc::Body(*lnum),
            alias: Some(name.to_string()),
        });
    }

    in_text_links
}

fn content_get_reference_list(line: &str, _id: &str, lnum: &usize) -> Vec<Ref> {
    static RE_REF: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\[([0-9]+)\]: (.+)").unwrap());
    let mut in_text_links = vec![];

    for (_, [ref_num, to_id]) in RE_REF.captures_iter(line).map(|c| c.extract()) {
        in_text_links.push(Ref {
            from: RefNode::Local(ref_num.parse().unwrap_or(0)),
            to: RefNode::Note(to_id.to_string()),
            loc: RefLoc::Body(*lnum),
            alias: None,
        });
    }

    in_text_links
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
        (ParseState::Nothing, true) => Ok(init_frontmatter_parse()),
        (ParseState::InYaml(front_matter_build), false) => Ok(ParseState::InYaml(
            add_line_to_frontmatter(line, front_matter_build),
        )),
        (ParseState::InYaml(front_matter_build), true) => format_frontmatter(front_matter_build),
        _ => Err(MyError::OtherError),
    }
}

fn init_frontmatter_parse() -> ParseState {
    ParseState::InYaml(FrontMatterBuild {
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
    let id = front_matter_build.id.ok_or(MyError::OtherError)?;
    Ok(ParseState::InMd(Note {
        id: id.clone(),
        aliases: front_matter_build.aliases,
        note_type: note_type_from_string(note_type)?,
        tags: front_matter_build
            .tags
            .unwrap_or(vec![])
            .iter()
            .map(|x| Ref {
                from: RefNode::Note(id.clone()),
                to: RefNode::Tag(x.to_string()),
                loc: RefLoc::FrontMatter,
                alias: None,
            })
            .collect(),
        references: front_matter_build
            .references
            .unwrap_or(vec![])
            .iter()
            .map(|x| Ref {
                from: RefNode::Note(id.clone()),
                to: RefNode::Note(x.to_string()),
                loc: RefLoc::FrontMatter,
                alias: None,
            })
            .collect(),
        source_parents: front_matter_build.source_parents.unwrap_or(vec![]),
        author: front_matter_build.author.unwrap_or(vec![]),
        contents: vec![],
        body_tags: vec![],
        body_links: vec![],
        frontmatter_lines: None,
        lines: None,
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
