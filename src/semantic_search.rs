//! Semantic note search: query vector, candidate retrieval, hybrid scoring,
//! assembly into hits, and keyword line targeting. Scoring helpers are pure
//! and unit-tested; database access goes through [`Db`] and the embedder is
//! injected, so tests need neither a model nor the network.

use std::collections::{HashMap, HashSet};

use duckdb::params_from_iter;

use crate::chunking::{ChunkKey, ChunkScope};
use crate::db::{Db, DbValue};
use crate::embed_store::vector_literal;
use crate::embeddings::Embedder;
use crate::{MyError, Result};

/// Share of the query vector taken from a context note.
const CONTEXT_SHARE: f32 = 0.3;
const SEMANTIC_CANDIDATES: usize = 100;
const KEYWORD_CANDIDATES: usize = 50;
const PSEUDO_SEEDS: usize = 3;
const MAX_TERMS: usize = 32;
const SNIPPET_CHARS: usize = 160;

const LINK_WEIGHT: f32 = 1.0;
const PARENT_WEIGHT: f32 = 0.8;
const SHARED_AUTHOR_WEIGHT: f32 = 0.6;

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "was", "with", "this",
    "that", "from", "have", "its", "into", "about", "what", "which", "who", "how", "why", "when",
    "where", "does", "did", "our", "out", "has", "had", "will", "would", "there", "their", "they",
    "of", "in", "on", "to", "is", "by", "be", "as", "at", "it", "or", "an",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub semantic: f32,
    pub keyword: f32,
    pub graph: f32,
}

impl Default for Weights {
    /// Prompt search. Graph is off: sweeping it against the evaluation sets
    /// lowered recall, because pseudo-seeds from semantic hits add noise.
    fn default() -> Self {
        Self {
            semantic: 0.6,
            keyword: 0.2,
            graph: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeightOverrides {
    pub semantic: Option<f32>,
    pub keyword: Option<f32>,
    pub graph: Option<f32>,
}

impl WeightOverrides {
    /// Applies any provided overrides on top of the query type's defaults.
    pub fn apply(self, base: Weights) -> Weights {
        Weights {
            semantic: self.semantic.unwrap_or(base.semantic),
            keyword: self.keyword.unwrap_or(base.keyword),
            graph: self.graph.unwrap_or(base.graph),
        }
    }
}

impl Weights {
    /// Note search has no prompt text, so the keyword signal is off and links
    /// carry the weight that semantic similarity does not.
    pub const fn note_default() -> Self {
        Self {
            semantic: 0.6,
            keyword: 0.0,
            graph: 0.4,
        }
    }

    /// Drops signals that do not exist for this query and rescales the rest
    /// so they still sum to one.
    pub fn effective(self, has_keyword: bool, has_graph: bool) -> Self {
        let keyword = if has_keyword { self.keyword } else { 0.0 };
        let graph = if has_graph { self.graph } else { 0.0 };
        let total = self.semantic + keyword + graph;
        if total <= 0.0 {
            return Self {
                semantic: 1.0,
                keyword: 0.0,
                graph: 0.0,
            };
        }
        Self {
            semantic: self.semantic / total,
            keyword: keyword / total,
            graph: graph / total,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Mixed,
    Anchor,
    Note,
}

impl Group {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mixed" => Some(Self::Mixed),
            "anchor" => Some(Self::Anchor),
            "note" => Some(Self::Note),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub limit: usize,
    pub group: Group,
    /// `None` means every type except `author`.
    pub types: Option<Vec<String>>,
    pub tags: Vec<String>,
    pub exclude_ids: Vec<String>,
    pub graph_seeds: bool,
    pub refine_line: bool,
    pub weights: WeightOverrides,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            limit: 20,
            group: Group::Mixed,
            types: None,
            tags: Vec::new(),
            exclude_ids: Vec::new(),
            graph_seeds: true,
            refine_line: true,
            weights: WeightOverrides::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Query {
    Prompt {
        text: String,
        context_note: Option<String>,
    },
    Note(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub note_id: String,
    pub anchor_id: Option<String>,
    pub title: String,
    pub heading: String,
    pub path: String,
    pub line: usize,
    pub chunk_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub score: f32,
    pub semantic: f32,
    pub keyword: f32,
    pub graph: f32,
    pub tags: Vec<String>,
    pub snippet: String,
}

impl Hit {
    pub fn to_db_value(&self) -> DbValue {
        let opt = |value: &Option<String>| match value {
            Some(text) => DbValue::String(text.clone()),
            None => DbValue::None,
        };
        DbValue::Table(vec![
            ("note_id".to_string(), DbValue::String(self.note_id.clone())),
            ("anchor_id".to_string(), opt(&self.anchor_id)),
            ("title".to_string(), DbValue::String(self.title.clone())),
            ("heading".to_string(), DbValue::String(self.heading.clone())),
            ("path".to_string(), DbValue::String(self.path.clone())),
            ("line".to_string(), DbValue::Int(self.line as i64)),
            (
                "chunk_line".to_string(),
                DbValue::Int(self.chunk_line as i64),
            ),
            ("start_col".to_string(), DbValue::Int(self.start_col as i64)),
            ("end_col".to_string(), DbValue::Int(self.end_col as i64)),
            ("score".to_string(), DbValue::Float(f64::from(self.score))),
            (
                "semantic".to_string(),
                DbValue::Float(f64::from(self.semantic)),
            ),
            (
                "keyword".to_string(),
                DbValue::Float(f64::from(self.keyword)),
            ),
            ("graph".to_string(), DbValue::Float(f64::from(self.graph))),
            (
                "tags".to_string(),
                DbValue::List(self.tags.iter().cloned().map(DbValue::String).collect()),
            ),
            ("snippet".to_string(), DbValue::String(self.snippet.clone())),
        ])
    }
}

/// Lowercased words without stop words, duplicates removed, capped at
/// [`MAX_TERMS`].
pub fn tokenize(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() > 1)
        .map(str::to_lowercase)
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
        .filter(|word| seen.insert(word.clone()))
        .take(MAX_TERMS)
        .collect()
}

/// `#tags` written in the prompt, without the `#`.
pub fn prompt_tags(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split_whitespace()
        .filter_map(|word| word.strip_prefix('#'))
        .map(|tag| {
            tag.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|tag| !tag.is_empty())
        .filter(|tag| seen.insert(tag.clone()))
        .collect()
}

/// Inverse document frequency, always non-negative (BM25 form).
pub fn idf(total: usize, df: usize) -> f32 {
    let total = total as f32;
    let df = df as f32;
    ((total - df + 0.5) / (df + 0.5)).ln_1p()
}

/// Probability that at least one independent edge fires.
pub fn noisy_or(weights: impl IntoIterator<Item = f32>) -> f32 {
    1.0 - weights
        .into_iter()
        .map(|w| 1.0 - w.clamp(0.0, 1.0))
        .product::<f32>()
}

/// IDF-weighted Jaccard similarity between two tag sets.
pub fn weighted_jaccard(
    a: &HashSet<String>,
    b: &HashSet<String>,
    weight: impl Fn(&str) -> f32,
) -> f32 {
    let inter: f32 = a.intersection(b).map(|t| weight(t)).sum();
    let union: f32 = a.union(b).map(|t| weight(t)).sum();
    if union <= 0.0 { 0.0 } else { inter / union }
}

pub fn normalize(vector: &[f32]) -> Vec<f32> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm <= 0.0 {
        return vector.to_vec();
    }
    vector.iter().map(|x| x / norm).collect()
}

/// Query vector for "prompt, with context note": 0.7 prompt + 0.3 note.
pub fn blend(prompt: &[f32], context: &[f32]) -> Vec<f32> {
    let prompt = normalize(prompt);
    let context = normalize(context);
    let mixed: Vec<f32> = prompt
        .iter()
        .zip(&context)
        .map(|(p, c)| (1.0 - CONTEXT_SHARE) * p + CONTEXT_SHARE * c)
        .collect();
    normalize(&mixed)
}

/// Parses a `FLOAT[]` rendered by DuckDB as text, e.g. `[0.1, -0.2]`.
pub fn parse_vector(text: &str) -> Result<Vec<f32>> {
    text.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| {
            item.trim()
                .parse::<f32>()
                .map_err(|error| MyError::String(format!("bad vector value: {error}")))
        })
        .collect()
}

/// Best line for the query: the one with the most term matches. Blank and
/// heading-only lines never win. Returns `None` when nothing matches.
pub fn best_line(lines: &[(usize, String)], terms: &[String]) -> Option<(usize, String)> {
    lines
        .iter()
        .filter(|(_, text)| !text.trim().is_empty() && !text.trim_start().starts_with('#'))
        .map(|(line, text)| {
            let lower = text.to_lowercase();
            let matches = terms
                .iter()
                .filter(|term| lower.contains(term.as_str()))
                .count();
            (matches, *line, text)
        })
        .filter(|(matches, _, _)| *matches > 0)
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, line, text)| (line, text.clone()))
}

#[derive(Debug, Clone)]
struct NoteInfo {
    title: String,
    note_type: String,
    path: String,
}

#[derive(Debug, Clone)]
struct Row {
    key: ChunkKey,
    line: Option<usize>,
    cos: f32,
    mask: u64,
}

/// Everything needed to score one query, loaded once per search.
#[derive(Debug, Default)]
struct Corpus {
    rows: Vec<Row>,
    term_df: Vec<usize>,
    notes: HashMap<String, NoteInfo>,
    tags: HashMap<String, HashSet<String>>,
    tag_df: HashMap<String, usize>,
    /// Link and parent edges, stored in both directions.
    adjacency: HashMap<String, Vec<(String, f32)>>,
    authors: HashMap<String, HashSet<String>>,
    anchor_titles: HashMap<String, String>,
}

impl Corpus {
    fn note_total(&self) -> usize {
        self.notes.len()
    }

    fn tag_weight(&self, tag: &str) -> f32 {
        idf(
            self.note_total(),
            self.tag_df.get(tag).copied().unwrap_or(0),
        )
    }

    /// Graph signal for one note: link/parent edges and shared authors to the
    /// seeds, plus IDF-weighted tag overlap with the other seeds and the
    /// prompt's `#tags`. A note never counts as a seed for itself.
    fn graph_score(&self, note: &str, seeds: &[String], prompt_tags: &[String]) -> f32 {
        let other_seeds: Vec<&String> = seeds.iter().filter(|seed| seed.as_str() != note).collect();
        let mut weights = Vec::new();
        let mut seed_tags: HashSet<String> = prompt_tags.iter().cloned().collect();
        for seed in &other_seeds {
            if let Some(edges) = self.adjacency.get(note) {
                weights.extend(
                    edges
                        .iter()
                        .filter(|(other, _)| other == *seed)
                        .map(|(_, w)| *w),
                );
            }
            let shared = match (self.authors.get(note), self.authors.get(*seed)) {
                (Some(a), Some(b)) => a.intersection(b).next().is_some(),
                _ => false,
            };
            if shared {
                weights.push(SHARED_AUTHOR_WEIGHT);
            }
            if let Some(tags) = self.tags.get(*seed) {
                seed_tags.extend(tags.iter().cloned());
            }
        }
        let empty = HashSet::new();
        let note_tags = self.tags.get(note).unwrap_or(&empty);
        weights.push(weighted_jaccard(note_tags, &seed_tags, |t| {
            self.tag_weight(t)
        }));
        noisy_or(weights)
    }
}

fn load_corpus(db: &Db, query: &[f32], terms: &[String]) -> Result<Corpus> {
    let mut params = vec![vector_literal(query)];
    let mask_terms: Vec<String> = terms
        .iter()
        .enumerate()
        .map(|(i, term)| {
            params.push(term.clone());
            format!("contains(lower(e.text), ?)::BIGINT * {}", 1u64 << i)
        })
        .collect();
    let mask = if mask_terms.is_empty() {
        "0::BIGINT".to_string()
    } else {
        mask_terms.join(" + ")
    };
    let rows_sql = format!(
        "select e.note_id, e.scope, e.anchor_id, e.part, e.line,
                list_cosine_similarity(e.vector, cast(? as FLOAT[])) as cos,
                {mask} as mask
         from embeddings e"
    );
    let rows = {
        let mut stmt = db.connection.prepare(&rows_sql)?;
        stmt.query_map(params_from_iter(&params), |row| {
            let scope: String = row.get(1)?;
            Ok(Row {
                key: ChunkKey {
                    note_id: row.get(0)?,
                    scope: if scope == "note" {
                        ChunkScope::Note
                    } else {
                        ChunkScope::Anchor
                    },
                    anchor_id: row.get(2)?,
                    part: row.get::<_, i32>(3)? as u32,
                },
                line: row.get::<_, Option<u32>>(4)?.map(|line| line as usize),
                cos: row.get::<_, Option<f32>>(5)?.unwrap_or(0.0),
                mask: row.get::<_, i64>(6)? as u64,
            })
        })?
        .collect::<duckdb::Result<Vec<Row>>>()?
    };

    let df_columns: Vec<String> = terms
        .iter()
        .map(|_| "coalesce(sum(contains(lower(e.text), ?)::INT), 0)::BIGINT".to_string())
        .collect();
    let term_df = if terms.is_empty() {
        Vec::new()
    } else {
        let df_sql = format!("select {} from embeddings e", df_columns.join(", "));
        let mut stmt = db.connection.prepare(&df_sql)?;
        let mut rows = stmt.query(params_from_iter(terms))?;
        let row = rows.next()?.ok_or(MyError::OtherError)?;
        (0..terms.len())
            .map(|i| row.get::<_, i64>(i).map(|v| v as usize))
            .collect::<duckdb::Result<Vec<usize>>>()?
    };

    let notes = db
        .query_all(
            "select note_id, note_title, note_type, path from notes",
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    NoteInfo {
                        title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        note_type: row.get(2)?,
                        path: row.get(3)?,
                    },
                ))
            },
        )?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let mut tags: HashMap<String, HashSet<String>> = HashMap::new();
    for (note, tag) in db.query_all("select note_id, tag from tags", |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        tags.entry(note).or_default().insert(tag);
    }
    let mut tag_df: HashMap<String, usize> = HashMap::new();
    for set in tags.values() {
        for tag in set {
            *tag_df.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    let anchor_rows = db.query_all("select anchor_id, note_id, title from anchors", |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let anchor_owner: HashMap<String, String> = anchor_rows
        .iter()
        .map(|(anchor, note, _)| (anchor.clone(), note.clone()))
        .collect();
    let anchor_titles: HashMap<String, String> = anchor_rows
        .into_iter()
        .map(|(anchor, _, title)| (anchor, title))
        .collect();

    let mut adjacency: HashMap<String, Vec<(String, f32)>> = HashMap::new();
    let mut authors: HashMap<String, HashSet<String>> = HashMap::new();
    for (note, kind, to_anchor) in
        db.query_all("select note_id, kind, to_anchor from refs", |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
    {
        let target = anchor_owner.get(&to_anchor).cloned().unwrap_or(to_anchor);
        let weight = match kind.as_str() {
            "intext_link" | "frontmatter_reference" => LINK_WEIGHT,
            "parent" => PARENT_WEIGHT,
            "author" => {
                authors.entry(note).or_default().insert(target);
                continue;
            }
            _ => continue,
        };
        adjacency
            .entry(note.clone())
            .or_default()
            .push((target.clone(), weight));
        adjacency.entry(target).or_default().push((note, weight));
    }

    Ok(Corpus {
        rows,
        term_df,
        notes,
        tags,
        tag_df,
        adjacency,
        authors,
        anchor_titles,
    })
}

fn stored_vector(db: &Db, note_id: &str) -> Result<Option<Vec<f32>>> {
    let text: Option<String> = match db.connection.query_row(
        "select cast(vector as VARCHAR) from embeddings
         where note_id = ? and scope = 'note' and anchor_id = '' and part = 0",
        [note_id],
        |row| row.get(0),
    ) {
        Ok(text) => Some(text),
        Err(duckdb::Error::QueryReturnedNoRows) => None,
        Err(error) => return Err(error.into()),
    };
    text.map(|text| parse_vector(&text)).transpose()
}

fn kw_raw(mask: u64, term_idf: &[f32]) -> f32 {
    term_idf
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1u64 << i) != 0)
        .map(|(_, weight)| weight)
        .sum()
}

#[derive(Debug, Clone, Copy)]
struct Scored {
    index: usize,
    score: f32,
    semantic: f32,
    keyword: f32,
    graph: f32,
}

/// Runs a search and returns hits ordered by score.
pub fn search(
    db: &Db,
    embedder: &dyn Embedder,
    query: &Query,
    opts: &SearchOpts,
) -> Result<Vec<Hit>> {
    let (terms, prompt_tags, query_vector, seeds, excluded_note) = match query {
        Query::Prompt { text, context_note } => {
            let terms = tokenize(text);
            let mut vector = embedder.embed_query(text)?;
            let seeds = match context_note {
                Some(note) => {
                    let context = stored_vector(db, note)?.ok_or_else(|| {
                        MyError::String(format!(
                            "context note '{note}' has no embedding; run a refresh"
                        ))
                    })?;
                    vector = blend(&vector, &context);
                    vec![note.clone()]
                }
                None => Vec::new(),
            };
            (terms, prompt_tags(text), vector, seeds, None)
        }
        Query::Note(note) => {
            let vector = stored_vector(db, note)?.ok_or_else(|| {
                MyError::String(format!("note '{note}' has no embedding; run a refresh"))
            })?;
            (
                Vec::new(),
                Vec::new(),
                vector,
                vec![note.clone()],
                Some(note.clone()),
            )
        }
    };

    let corpus = load_corpus(db, &query_vector, &terms)?;

    let seeds = if seeds.is_empty() && opts.graph_seeds && matches!(query, Query::Prompt { .. }) {
        top_notes(&corpus, PSEUDO_SEEDS)
    } else {
        seeds
    };
    for seed in &seeds {
        if !corpus.notes.contains_key(seed) {
            return Err(MyError::String(format!("unknown note '{seed}'")));
        }
    }

    let has_keyword = !terms.is_empty();
    let has_graph = !seeds.is_empty() || !prompt_tags.is_empty();
    let base = match query {
        Query::Prompt { .. } => Weights::default(),
        Query::Note(_) => Weights::note_default(),
    };
    let weights = opts.weights.apply(base).effective(has_keyword, has_graph);

    let term_idf: Vec<f32> = corpus
        .term_df
        .iter()
        .map(|df| idf(corpus.rows.len(), *df))
        .collect();

    let allowed = |row: &Row| -> bool {
        let note_id = &row.key.note_id;
        let Some(info) = corpus.notes.get(note_id) else {
            return false;
        };
        if Some(note_id) == excluded_note.as_ref() || opts.exclude_ids.contains(note_id) {
            return false;
        }
        let type_ok = match &opts.types {
            Some(types) => types.contains(&info.note_type),
            None => info.note_type != "author",
        };
        let tags_ok = opts.tags.is_empty()
            || corpus
                .tags
                .get(note_id)
                .is_some_and(|tags| opts.tags.iter().any(|t| tags.contains(t)));
        type_ok && tags_ok
    };

    let pool: Vec<usize> = (0..corpus.rows.len())
        .filter(|i| allowed(&corpus.rows[*i]))
        .collect();

    let mut by_semantic = pool.clone();
    by_semantic.sort_by(|a, b| corpus.rows[*b].cos.total_cmp(&corpus.rows[*a].cos));
    by_semantic.truncate(SEMANTIC_CANDIDATES);

    let mut by_keyword: Vec<(usize, f32)> = pool
        .iter()
        .map(|i| (*i, kw_raw(corpus.rows[*i].mask, &term_idf)))
        .filter(|(_, raw)| *raw > 0.0)
        .collect();
    by_keyword.sort_by(|a, b| b.1.total_cmp(&a.1));
    by_keyword.truncate(KEYWORD_CANDIDATES);

    let mut candidates: Vec<usize> = Vec::new();
    let mut seen = HashSet::new();
    for index in by_semantic
        .into_iter()
        .chain(by_keyword.iter().map(|(i, _)| *i))
    {
        if seen.insert(index) {
            candidates.push(index);
        }
    }

    let raw: Vec<f32> = candidates
        .iter()
        .map(|i| kw_raw(corpus.rows[*i].mask, &term_idf))
        .collect();
    let max_raw = raw.iter().copied().fold(0.0f32, f32::max);

    let mut graph_cache: HashMap<String, f32> = HashMap::new();
    let mut scored: Vec<Scored> = Vec::with_capacity(candidates.len());
    for (position, index) in candidates.iter().enumerate() {
        let row = &corpus.rows[*index];
        let semantic = row.cos.clamp(0.0, 1.0);
        let keyword = if max_raw > 0.0 {
            raw[position] / max_raw
        } else {
            0.0
        };
        let graph = *graph_cache
            .entry(row.key.note_id.clone())
            .or_insert_with(|| corpus.graph_score(&row.key.note_id, &seeds, &prompt_tags));
        let score = weights.semantic * semantic + weights.keyword * keyword + weights.graph * graph;
        scored.push(Scored {
            index: *index,
            score,
            semantic,
            keyword,
            graph,
        });
    }

    let selected = assemble(&corpus, &scored, opts);

    let mut hits = Vec::with_capacity(selected.len());
    for item in selected {
        hits.push(build_hit(db, &corpus, item, &terms, opts)?);
    }
    Ok(hits)
}

/// Distinct notes with the highest semantic similarity, used as seeds when a
/// prompt has no context note.
fn top_notes(corpus: &Corpus, count: usize) -> Vec<String> {
    let mut best: HashMap<&str, f32> = HashMap::new();
    for row in &corpus.rows {
        let entry = best.entry(row.key.note_id.as_str()).or_insert(f32::MIN);
        *entry = entry.max(row.cos);
    }
    let mut ranked: Vec<(&str, f32)> = best.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
        .into_iter()
        .take(count)
        .map(|(note, _)| note.to_string())
        .collect()
}

/// Groups scored chunks into the requested result shape. Anchors keep their
/// best chunk; a note row is kept only when its best chunk is the note itself,
/// otherwise the anchor row already covers it.
fn assemble(corpus: &Corpus, scored: &[Scored], opts: &SearchOpts) -> Vec<Scored> {
    let mut best_anchor: HashMap<(String, String), Scored> = HashMap::new();
    let mut best_note: HashMap<String, Scored> = HashMap::new();

    for item in scored {
        let row = &corpus.rows[item.index];
        if row.key.scope == ChunkScope::Anchor {
            let key = (row.key.note_id.clone(), row.key.anchor_id.clone());
            let entry = best_anchor.entry(key).or_insert(*item);
            if item.score > entry.score {
                *entry = *item;
            }
        }
        let entry = best_note.entry(row.key.note_id.clone()).or_insert(*item);
        if item.score > entry.score {
            *entry = *item;
        }
    }

    let mut out: Vec<Scored> = match opts.group {
        Group::Anchor => best_anchor.into_values().collect(),
        Group::Note => best_note.into_values().collect(),
        Group::Mixed => {
            let mut items: Vec<Scored> = best_anchor.into_values().collect();
            items.extend(
                best_note
                    .into_values()
                    .filter(|item| corpus.rows[item.index].key.scope == ChunkScope::Note),
            );
            items
        }
    };
    out.retain(|item| item.score > 0.0);
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(opts.limit);
    out
}

fn build_hit(
    db: &Db,
    corpus: &Corpus,
    item: Scored,
    terms: &[String],
    opts: &SearchOpts,
) -> Result<Hit> {
    let row = &corpus.rows[item.index];
    let note = corpus.notes.get(&row.key.note_id);
    let anchor_id = match row.key.scope {
        ChunkScope::Anchor => Some(row.key.anchor_id.clone()),
        ChunkScope::Note => None,
    };
    let heading = anchor_id
        .as_ref()
        .and_then(|id| corpus.anchor_titles.get(id))
        .cloned()
        .unwrap_or_default();

    let chunk_line = row.line.unwrap_or(1);
    let mut line = chunk_line;
    let mut snippet = snippet_of(&chunk_text(db, &row.key)?);
    let start_col = 0;
    let mut end_col = 0;

    if row.key.scope == ChunkScope::Anchor
        && opts.refine_line
        && !terms.is_empty()
        && let Some((best, text)) = refine_line(db, &row.key.note_id, chunk_line, terms)?
    {
        line = best;
        end_col = text.len();
        snippet = text.trim().to_string();
    }

    let mut tags: Vec<String> = corpus
        .tags
        .get(&row.key.note_id)
        .map(|set| set.iter().cloned().collect())
        .unwrap_or_default();
    tags.sort();

    Ok(Hit {
        note_id: row.key.note_id.clone(),
        anchor_id,
        title: note.map(|n| n.title.clone()).unwrap_or_default(),
        heading,
        path: note.map(|n| n.path.clone()).unwrap_or_default(),
        line,
        chunk_line,
        start_col,
        end_col,
        score: item.score,
        semantic: item.semantic,
        keyword: item.keyword,
        graph: item.graph,
        tags,
        snippet,
    })
}

fn chunk_text(db: &Db, key: &ChunkKey) -> Result<String> {
    Ok(db.connection.query_row(
        "select text from embeddings
         where note_id = ? and scope = ? and anchor_id = ? and part = ?",
        duckdb::params![key.note_id, key.scope.as_str(), key.anchor_id, key.part],
        |row| row.get(0),
    )?)
}

fn snippet_of(text: &str) -> String {
    let body = text.split_once("\n\n").map_or(text, |(_, body)| body);
    body.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(SNIPPET_CHARS)
        .collect()
}

/// Stage 3: the line inside the anchor's section that best matches the query.
fn refine_line(
    db: &Db,
    note_id: &str,
    section_line: usize,
    terms: &[String],
) -> Result<Option<(usize, String)>> {
    let mut stmt = db.connection.prepare(
        "select line, text from contents
         where note_id = ? and line >= ?
           and line < coalesce((select min(line) from anchors where note_id = ? and line > ?), 1000000000)
         order by line",
    )?;
    let lines = stmt
        .query_map(
            duckdb::params![note_id, section_line as u32, note_id, section_line as u32],
            |row| Ok((row.get::<_, u32>(0)? as usize, row.get::<_, String>(1)?)),
        )?
        .collect::<duckdb::Result<Vec<_>>>()?;
    Ok(best_line(&lines, terms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_drops_stop_words_short_words_and_duplicates() {
        assert_eq!(
            tokenize("The Marx and marx value of a reification"),
            vec!["marx", "value", "reification"]
        );
    }

    #[test]
    fn prompt_tags_are_read_from_hash_words() {
        assert_eq!(
            prompt_tags("ideas #Topic and #topic-two, #"),
            vec!["topic", "topic-two"]
        );
    }

    #[test]
    fn rarer_terms_weigh_more() {
        assert!(idf(100, 1) > idf(100, 50));
        assert!(idf(100, 100) >= 0.0);
    }

    #[test]
    fn noisy_or_combines_independent_edges() {
        assert_eq!(noisy_or(Vec::<f32>::new()), 0.0);
        assert!((noisy_or([0.5, 0.5]) - 0.75).abs() < 1e-6);
        assert_eq!(noisy_or([1.0, 0.2]), 1.0);
    }

    #[test]
    fn weights_redistribute_missing_signals() {
        let effective = Weights::default().effective(false, true);
        assert!((effective.semantic + effective.graph - 1.0).abs() < 1e-6);
        assert_eq!(effective.keyword, 0.0);
        let semantic_only = Weights::default().effective(false, false);
        assert_eq!(semantic_only.semantic, 1.0);
        let all = Weights::default().effective(true, true);
        assert!((all.semantic + all.keyword + all.graph - 1.0).abs() < 1e-6);
    }

    #[test]
    fn weighted_jaccard_uses_weights() {
        let a: HashSet<String> = ["rare", "common"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["rare"].iter().map(|s| s.to_string()).collect();
        let weight = |t: &str| if t == "rare" { 3.0 } else { 1.0 };
        assert!((weighted_jaccard(&a, &b, weight) - 0.75).abs() < 1e-6);
        assert_eq!(
            weighted_jaccard(&HashSet::new(), &HashSet::new(), weight),
            0.0
        );
    }

    #[test]
    fn blend_favours_the_prompt() {
        let blended = blend(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(blended[0] > blended[1]);
        let norm: f32 = blended.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parses_duckdb_vector_text() {
        assert_eq!(
            parse_vector("[0.5, -1, 2.25]").unwrap(),
            vec![0.5, -1.0, 2.25]
        );
        assert!(parse_vector("[x]").is_err());
    }

    #[test]
    fn best_line_prefers_matches_and_skips_headings() {
        let lines = vec![
            (3, "# Marx heading".to_string()),
            (4, "".to_string()),
            (5, "plain sentence".to_string()),
            (6, "value and reification here".to_string()),
            (7, "value only".to_string()),
        ];
        let terms = vec!["value".to_string(), "reification".to_string()];
        assert_eq!(
            best_line(&lines, &terms),
            Some((6, "value and reification here".to_string()))
        );
        assert_eq!(best_line(&lines, &["nothing".to_string()]), None);
    }

    #[test]
    fn snippet_skips_the_header_block() {
        let text = "Title > Heading\n\nFirst   line\nsecond line";
        assert_eq!(snippet_of(text), "First line second line");
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;
    use crate::embed_index::update_embeddings;
    use crate::embeddings::FakeEmbedder;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn note(id: &str, note_type: &str, tags: &str, body: &str) -> String {
        format!("---\nid: {id}\naliases: {id}\ntype: {note_type}\ntags: [{tags}]\n---\n{body}")
    }

    /// Builds a small vault, indexes it, and returns (db, notes dir, db file).
    pub(super) fn vault() -> (Db, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "mesh-search-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let notes = base.join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::create_dir_all(base.join("db")).unwrap();
        let db_file = base.join("db").join("vault.duckdb");

        fs::write(
            notes.join("alpha.md"),
            note(
                "Alpha",
                "note",
                "marx",
                "# Alpha\nMarx theory of value and reification.\n## Details\nThe commodity form and fetishism.\n",
            ),
        )
        .unwrap();
        fs::write(
            notes.join("beta.md"),
            note(
                "Beta",
                "note",
                "cooking",
                "# Beta\nPasta recipes with garlic and oil.\n",
            ),
        )
        .unwrap();
        fs::write(
            notes.join("gamma.md"),
            note(
                "Gamma",
                "note",
                "marx",
                "# Gamma\nNotes on [[Alpha]] and value.\n",
            ),
        )
        .unwrap();
        fs::write(
            notes.join("doe.md"),
            note("a_doe", "author", "", "# Doe\nMarx scholar biography.\n"),
        )
        .unwrap();

        {
            let db = Db::init(&notes, &db_file).unwrap();
            db.process_files().unwrap();
        }
        update_embeddings(&FakeEmbedder::standard(), &notes, &db_file).unwrap();
        let db = Db::init(&notes, &db_file).unwrap();
        (db, notes, db_file)
    }

    fn prompt(text: &str) -> Query {
        Query::Prompt {
            text: text.to_string(),
            context_note: None,
        }
    }

    #[test]
    fn prompt_ranks_the_matching_note_first() {
        let (db, _, _) = vault();
        let hits = search(
            &db,
            &FakeEmbedder::standard(),
            &prompt("marx value reification"),
            &SearchOpts::default(),
        )
        .unwrap();
        let alpha = hits
            .iter()
            .find(|h| h.note_id == "Alpha")
            .expect("Alpha is relevant");
        assert!(alpha.keyword > 0.0);
        assert!(hits.iter().take(3).any(|h| h.note_id == "Alpha"));
        assert!(hits.iter().all(|h| h.score > 0.0));
    }

    #[test]
    fn authors_are_excluded_unless_asked_for() {
        let (db, _, _) = vault();
        let embedder = FakeEmbedder::standard();
        let default = search(
            &db,
            &embedder,
            &prompt("marx scholar"),
            &SearchOpts::default(),
        )
        .unwrap();
        assert!(default.iter().all(|h| h.note_id != "a_doe"));

        let opts = SearchOpts {
            types: Some(vec!["author".to_string()]),
            ..SearchOpts::default()
        };
        let authors = search(&db, &embedder, &prompt("marx scholar"), &opts).unwrap();
        assert!(authors.iter().any(|h| h.note_id == "a_doe"));
    }

    #[test]
    fn note_query_excludes_the_note_and_uses_links() {
        let (db, _, _) = vault();
        let hits = search(
            &db,
            &FakeEmbedder::standard(),
            &Query::Note("Gamma".to_string()),
            &SearchOpts::default(),
        )
        .unwrap();
        assert!(hits.iter().all(|h| h.note_id != "Gamma"));
        let alpha = hits
            .iter()
            .find(|h| h.note_id == "Alpha")
            .expect("Alpha is linked from Gamma");
        assert!(alpha.graph > 0.0);
        assert_eq!(alpha.keyword, 0.0, "note queries have no keyword signal");
    }

    #[test]
    fn anchor_hit_points_at_the_matching_line() {
        let (db, _, _) = vault();
        let opts = SearchOpts {
            group: Group::Anchor,
            ..SearchOpts::default()
        };
        let hits = search(
            &db,
            &FakeEmbedder::standard(),
            &prompt("reification"),
            &opts,
        )
        .unwrap();
        let alpha = hits
            .iter()
            .find(|h| h.anchor_id.as_deref() == Some("Alpha"))
            .expect("Alpha section matches");
        assert_eq!(alpha.line, 8, "line 8 holds the reification sentence");
        assert_eq!(alpha.chunk_line, 7, "section starts at the heading");
        assert!(alpha.snippet.contains("reification"));
    }

    #[test]
    fn mixed_groups_do_not_repeat_a_note_whose_anchor_is_listed() {
        let (db, _, _) = vault();
        let hits = search(
            &db,
            &FakeEmbedder::standard(),
            &prompt("marx value"),
            &SearchOpts::default(),
        )
        .unwrap();
        let mut keys: Vec<(String, Option<String>)> = hits
            .iter()
            .map(|h| (h.note_id.clone(), h.anchor_id.clone()))
            .collect();
        let total = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), total, "no (note, anchor) pair is listed twice");
    }

    #[test]
    fn a_note_does_not_boost_itself_through_its_own_tags() {
        let (db, _, _) = vault();
        let opts = SearchOpts {
            group: Group::Note,
            ..SearchOpts::default()
        };
        let query = Query::Prompt {
            text: "pasta".to_string(),
            context_note: Some("Beta".to_string()),
        };
        let hits = search(&db, &FakeEmbedder::standard(), &query, &opts).unwrap();
        let beta = hits
            .iter()
            .find(|h| h.note_id == "Beta")
            .expect("Beta matches pasta");
        assert_eq!(beta.graph, 0.0);
    }

    #[test]
    fn context_note_is_used_and_unknown_context_is_an_error() {
        let (db, _, _) = vault();
        let embedder = FakeEmbedder::standard();
        let with_context = Query::Prompt {
            text: "value".to_string(),
            context_note: Some("Gamma".to_string()),
        };
        assert!(
            !search(&db, &embedder, &with_context, &SearchOpts::default())
                .unwrap()
                .is_empty()
        );

        let missing = Query::Prompt {
            text: "value".to_string(),
            context_note: Some("Nope".to_string()),
        };
        assert!(search(&db, &embedder, &missing, &SearchOpts::default()).is_err());
    }

    #[test]
    fn tag_filter_restricts_results() {
        let (db, _, _) = vault();
        let opts = SearchOpts {
            tags: vec!["cooking".to_string()],
            ..SearchOpts::default()
        };
        let hits = search(
            &db,
            &FakeEmbedder::standard(),
            &prompt("marx value pasta"),
            &opts,
        )
        .unwrap();
        assert!(hits.iter().all(|h| h.note_id == "Beta"));
    }
}
