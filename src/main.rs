use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::{self};
use std::io::{self, BufRead, Lines};
use std::path::Path;
use std::sync::LazyLock;
use std::time::Instant;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("A custom error occurred")]
    OtherError,
}

#[derive(Clone, Debug)]
enum NotesPath {
    File(Note),
    Dir(Vec<NotesPath>),
}

#[derive(Serialize, Deserialize)]
struct Output {
    notes: Vec<Note>,
    references: Vec<Ref>,
}

#[derive(Serialize, Deserialize)]
struct OutputToo {
    notes: Vec<NoteOutput>,
    references: Vec<RefOutput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Note {
    id: String,
    aliases: Option<String>,
    note_type: NoteType,
    tags: Vec<Ref>,
    links: Vec<Ref>,
    contents: Vec<ContentLine>,
    source_parents: Vec<String>,
    author: Vec<String>,
    lines: usize,
    frontmatter_lines: usize,
}

#[derive(Serialize, Deserialize)]
struct NoteOutput {
    id: String,
    aliases: Option<String>,
    note_type: String,
    tags: Vec<String>,
    links: Vec<String>,
    // contents: Vec<ContentLine>,
    source_parents: Vec<String>,
    author: Vec<String>,
    lines: usize,
    frontmatter_lines: usize,
}

impl Ref {
    fn to_output_name(&self) -> String {
        match self.to.clone() {
            RefNode::Note(y) => y,
            RefNode::Tag(y) => y,
            RefNode::Local(_) => "".to_string(),
        }
    }

    fn to_output_struct(self) -> RefOutput {
        let (ref_type, to) = match self.to {
            RefNode::Note(x) => ("note".to_string(), x),
            RefNode::Tag(x) => ("tag".to_string(), x),
            RefNode::Local(x) => ("local".to_string(), x.to_string()),
        };
        let from = match self.from {
            RefNode::Note(x) => x,
            RefNode::Tag(x) => x,
            RefNode::Local(x) => x.to_string(),
        };
        let line = match self.loc {
            RefLoc::FrontMatter => 0,
            RefLoc::Body(x) => x,
        };

        RefOutput { from, to, ref_type, line, alias: self.alias }
    }
}
impl Note {
    fn to_output(self) -> NoteOutput {
        let note_type = match self.note_type {
            NoteType::Source => "source",
            NoteType::Author => "author",
            NoteType::Note => "note",
        }
        .to_string();

        let tags = self.tags.iter().map(|x| x.clone().to_output_name()).collect();
        let links = self.links.iter().map(|x| x.clone().to_output_name()).collect();

        NoteOutput {
            id: self.id,
            aliases: self.aliases,
            note_type,
            tags,
            links,
            source_parents: self.source_parents,
            author: self.author,
            lines: self.lines,
            frontmatter_lines: self.frontmatter_lines,
        }
    }
}

#[derive(Clone, Debug)]
struct InterimNote {
    id: String,
    aliases: Option<String>,
    note_type: NoteType,
    tags: Vec<Ref>,
    references: Vec<Ref>,
    source_parents: Vec<String>,
    author: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Ref {
    from: RefNode,
    to: RefNode,
    loc: RefLoc,
    alias: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RefOutput {
    from: String,
    to: String,
    ref_type: String,
    line: usize,
    alias: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum RefNode {
    Note(String),
    Tag(String),
    Local(usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum RefLoc {
    FrontMatter,
    Body(usize),
}

pub type Result<T> = std::result::Result<T, MyError>;

// #[derive(Debug)]
enum ParseState {
    Nothing,
    InYaml(FrontMatterBuild),
    InMd(InterimNote),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContentLine {
    text: String,
    line_num: usize,
    note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum NoteType {
    Source,
    Author,
    Note,
}

fn get_notes(test: Vec<NotesPath>) -> Vec<Note> {
    let mut res = vec![];
    for x in test {
        match x {
            NotesPath::File(note) => res.push(note),
            NotesPath::Dir(notes_paths) => res.append(&mut get_notes(notes_paths)),
        }
    }

    res
}

fn get_note_outputs(test: Vec<NotesPath>) -> Vec<NoteOutput> {
    let mut res = vec![];
    for x in test {
        match x {
            NotesPath::File(note) => res.push(note.to_output()),
            NotesPath::Dir(notes_paths) => res.append(&mut get_note_outputs(notes_paths)),
        }
    }

    res
}

fn get_refs(test: &[Note]) -> Vec<Ref> {
    let mut res = vec![];

    test.iter().for_each(|x| {
        res.extend(x.tags.clone());
        res.extend(x.links.clone());
    });

    res
}


fn get_outputs(test: &[Note]) -> Vec<RefOutput> {
    let mut res = vec![];

    test.iter().for_each(|x| {
        x.tags.iter().for_each(|y| res.push(y.clone().to_output_struct()));
        x.links.iter().for_each(|y| res.push(y.clone().to_output_struct()));
    });

    res
}

fn main() -> Result<()> {
    let cache = Path::new("../testing/zettel/cache.json");
    // // Read the current directory (".").
    // let s = Instant::now();
    // let main_dir = Path::new("../testing/zettel/");
    // let test = read_dir(main_dir)?;
    // let x = get_notes(test);
    // // let y = get_refs(&x);
    //
    // // let out = Output {
    // //     references: y,
    // //     notes: x,
    // // };
    //
    // // let s = Instant::now();
    // // // println!("{}", serde_json::to_string(&out)?);
    // // let normal = s.elapsed();
    //
    // // let s2 = Instant::now();
    // //
    // // let aa = s2.elapsed();
    // //
    // // let out = OutputToo {
    // //     references: y,
    // //     notes: x,
    // // };
    //
    // // println!("{}", serde_json::to_string(&out)?);
    //
    // //
    // std::fs::write(cache, serde_json::to_string(&x)?)?;
    // //
    // let write = s.elapsed();
    //
    let s = Instant::now();
    let x: Vec<Note> = serde_json::from_str(&std::fs::read_to_string(cache)?)?;

    let y = get_refs(&x);
    // let notes = x.iter().map(|y| y.clone().to_output()).collect();

    // let out = OutputToo {
    //     references: y,
    //     notes,
    // };

    // let backlinks: Vec<RefOutput> = y.iter().filter_map(|x| {
    //     match x.clone().to {
    //         RefNode::Note(y) if y == "s_chambers_capitalist-economics_bankers-interest" => Some(x.clone().to_output_struct()),
    //         _ => None,
    //     }
    // }).collect();

    let contents: Vec<ContentLine> = x.iter().flat_map(|x| x.clone().contents).collect();

    let read = s.elapsed();
    println!("{}", serde_json::to_string_pretty(&contents)?);
    println!("read: {:#?}", read);

    Ok(())
}

pub enum FoldState<T> {
    Continue(T),
    Break(T),
}

fn read_file(file: &Path) -> Result<Note> {
    if file.extension().and_then(|x| x.to_str()).unwrap_or("") != "md" {
        return Err(MyError::OtherError);
    }

    let start = Instant::now();
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

    let note = match parsed {
        ParseState::InMd(note) => Ok(note),
        _ => Err(MyError::OtherError),
    }?;

    let mut parser = BodyParsing::init(note, frontmatter_line_num);

    parser.parse_lines(rest);

    let note = parser.consume();

    let duration = start.elapsed();
    // println!("Time for parsing note: {:?}", duration);

    Ok(note)
}

struct BodyParsing {
    id: String,
    contents: Vec<ContentLine>,
    tags: Vec<Ref>,
    links: Vec<Ref>,
    refs: Vec<Ref>,
    ref_list: Vec<Ref>,
    line_num: usize,
    aliases: Option<String>,
    note_type: NoteType,
    references: Vec<Ref>,
    source_parents: Vec<String>,
    author: Vec<String>,
    frontmatter_lines: usize,
}

impl BodyParsing {
    fn init(note: InterimNote, frontmatter_line_num: usize) -> Self {
        BodyParsing {
            id: note.id,
            contents: vec![],
            tags: note.tags,
            links: vec![],
            line_num: frontmatter_line_num,
            aliases: note.aliases,
            note_type: note.note_type,
            refs: vec![],
            ref_list: vec![],
            references: note.references,
            source_parents: note.source_parents,
            author: note.author,
            frontmatter_lines: frontmatter_line_num,
        }
    }

    fn consume(mut self) -> Note {
        self.ref_list.sort_by_key(|x| match x.from {
            RefNode::Note(_) => 999,
            RefNode::Tag(_) => 999,
            RefNode::Local(x) => x,
        });

        self.refs.iter().for_each(|x| {
            self.links.push(Ref {
                from: x.from.clone(),
                to: {
                    match x.to.clone() {
                        RefNode::Local(y) => self.ref_list[y].to.clone(),
                        z => z,
                    }
                },
                loc: x.loc.clone(),
                alias: x.alias.clone(),
            })
        });

        self.links.append(&mut self.refs);
        self.links.append(&mut self.references);
        Note {
            id: self.id,
            contents: self.contents,
            tags: self.tags,
            links: self.links,
            aliases: self.aliases,
            note_type: self.note_type,
            source_parents: self.source_parents,
            author: self.author,
            lines: self.line_num,
            frontmatter_lines: self.frontmatter_lines,
        }
    }

    fn parse_lines(&mut self, rest: Lines<std::io::BufReader<std::fs::File>>) {
        // let start = Instant::now();
        rest.for_each(|x| self.parse_line(x));
        // let duration = start.elapsed();
        // println!("Time for parsing body: {:?}", duration);
    }

    fn parse_line(&mut self, text: std::result::Result<String, io::Error>) {
        let text = text.unwrap_or("".to_string());
        self.get_tags(&text);
        self.get_links(&text);
        self.get_reference_intext(&text);
        self.get_reference_list(&text);
        self.contents.push(ContentLine {
            note: self.id.clone(),
            text,
            line_num: self.line_num,
        });
        self.line_num += 1;
    }

    fn get_tags(&mut self, line: &String) {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#([a-zA-Z\-]+)").unwrap());

        for (_, [tag]) in RE.captures_iter(&line).map(|c| c.extract()) {
            self.tags.push(Ref {
                from: RefNode::Note(self.id.clone()),
                to: RefNode::Tag(tag.to_string()),
                loc: RefLoc::Body(self.line_num),
                alias: None,
            });
        }
    }

    fn get_links(&mut self, line: &String) {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[\[([^\|\]]+)\|?([^\|\]]*)\]\]").unwrap());

        for (_, [to_id, title]) in RE.captures_iter(&line).map(|c| c.extract()) {
            self.links.push(Ref {
                from: RefNode::Note(self.id.clone()),
                to: RefNode::Note(to_id.to_string()),
                loc: RefLoc::Body(self.line_num),
                alias: Some(title.to_string()),
            });
        }
    }

    fn get_reference_intext(&mut self, line: &String) {
        static RE_INTEXT: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[([0-9]+)\]").unwrap());
        for (_, [name, ref_num]) in RE_INTEXT.captures_iter(&line).map(|c| c.extract()) {
            self.refs.push(Ref {
                from: RefNode::Note(self.id.clone()),
                to: RefNode::Local(ref_num.parse().unwrap_or(0)),
                loc: RefLoc::Body(self.line_num),
                alias: Some(name.to_string()),
            });
        }
    }

    fn get_reference_list(&mut self, line: &String) {
        static RE_REF: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\[([0-9]+)\]: (.+)").unwrap());
        for (_, [ref_num, to_id]) in RE_REF.captures_iter(&line).map(|c| c.extract()) {
            self.ref_list.push(Ref {
                from: RefNode::Local(ref_num.parse().unwrap_or(0)),
                to: RefNode::Note(to_id.to_string()),
                loc: RefLoc::Body(self.line_num),
                alias: None,
            });
        }
    }
}

// fn parse_body(
//     content: BodyParsing,
//     z: std::result::Result<String, io::Error>,
//     note_id: &str,
// ) -> BodyParsing {
//     let text = z.unwrap_or("".to_string());
//     let line_num = line_num + 1;
//
//     let start = Instant::now();
//     let (tags, links, n_refs, n_ref_list) = parse_from_line(&text, note_id, &line_num);
//     let duration = start.elapsed();
//     println!("Time to parse line: {:?}", duration);
//
//     let start = Instant::now();
//     let body_tags = app(body_tags, tags);
//     let body_links = app(body_links, links);
//     let refs = app(refs, n_refs);
//     let ref_list = app(ref_list, n_ref_list);
//     let contents = push(
//         contents,
//         ContentLine {
//             note: note_id.to_string(),
//             text,
//             line_num,
//         },
//     );
//     let duration = start.elapsed();
//     println!("Time to add to vectors: {:?}", duration);
//
//     (contents, body_tags, body_links, refs, ref_list, line_num)
// }
//
fn app<T>(mut first: Vec<T>, mut second: Vec<T>) -> Vec<T> {
    first.append(&mut second);
    first
}

// fn app_refs(mut first: Vec<Ref>, mut second: InterimNote) -> Vec<Ref> {
//     first.append(&mut second.tags);
//     first.append(&mut second.links);
//     first.append(&mut second.references);
//     first
// }

fn push<T>(mut first: Vec<T>, second: T) -> Vec<T> {
    first.push(second);
    first
}

// fn parse_from_line(line: &str, id: &str, lnum: &usize) -> (Vec<Ref>, Vec<Ref>, Vec<Ref>, Vec<Ref>) {
//     let tags = content_get_tags(line, id, lnum);
//     let links = content_get_links(line, id, lnum);
//     let refs_intext = content_get_reference_intext(line, id, lnum);
//     let refs_list = content_get_reference_list(line, id, lnum);
//     (tags, links, refs_intext, refs_list)
// }

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
    Ok(ParseState::InMd(InterimNote {
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
