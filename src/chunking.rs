//! Pure chunk construction for semantic search. No database access here: the
//! caller loads [`NoteSource`] values and receives [`ChunkJob`]s back.

pub const MAX_WINDOW_CHARS: usize = 1500;
pub const OVERLAP_CHARS: usize = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkScope {
    Note,
    Anchor,
}

impl ChunkScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Anchor => "anchor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub note_id: String,
    pub scope: ChunkScope,
    pub anchor_id: String,
    pub part: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkJob {
    pub key: ChunkKey,
    pub line: Option<usize>,
    pub text: String,
    pub text_hash: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NoteSource {
    pub id: String,
    pub title: String,
    /// Title of the parent note (for chapters of a book), when there is one.
    pub parent_title: Option<String>,
    pub note_type: String,
    pub tags: Vec<String>,
    pub authors: Vec<String>,
    pub anchors: Vec<AnchorSource>,
    /// Body lines as `(1-based line, text)`, sorted by line.
    pub lines: Vec<(usize, String)>,
}

#[derive(Debug, Clone)]
pub struct AnchorSource {
    pub anchor_id: String,
    pub heading: String,
    pub line: usize,
}

impl ChunkJob {
    fn new(key: ChunkKey, line: Option<usize>, text: String) -> Self {
        let text_hash = rapidhash::v3::rapidhash_v3(text.as_bytes());
        Self {
            key,
            line,
            text,
            text_hash,
        }
    }
}

/// One note-level chunk per note, plus one chunk per window of each non-empty
/// heading section. Author notes only get the note-level chunk.
pub fn build_chunks(notes: &[NoteSource]) -> Vec<ChunkJob> {
    let mut jobs = Vec::new();

    for note in notes {
        let body = note
            .lines
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let body_head = windows(&body, MAX_WINDOW_CHARS, OVERLAP_CHARS)
            .into_iter()
            .next()
            .unwrap_or_default();
        let mut header = format!(
            "{}\nType: {}\nTags: {}\nAuthors: {}",
            note.title,
            note.note_type,
            note.tags.join(", "),
            note.authors.join(", ")
        );
        if let Some(parent) = &note.parent_title {
            header.push_str(&format!("\nParent: {parent}"));
        }
        jobs.push(ChunkJob::new(
            ChunkKey {
                note_id: note.id.clone(),
                scope: ChunkScope::Note,
                anchor_id: String::new(),
                part: 0,
            },
            None,
            format!("{header}\n\n{body_head}").trim().to_string(),
        ));

        if note.note_type == "author" {
            continue;
        }

        let mut anchors = note.anchors.clone();
        anchors.sort_by_key(|anchor| anchor.line);
        for (index, anchor) in anchors.iter().enumerate() {
            let end = anchors.get(index + 1).map_or(usize::MAX, |next| next.line);
            let section = note
                .lines
                .iter()
                .filter(|(line, _)| *line >= anchor.line && *line < end)
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if section.trim().is_empty() {
                continue;
            }

            // Chapters often never name their own book. Include the parent title
            // so a section chunk carries the vocabulary of the work it belongs to.
            let prefix = match &note.parent_title {
                Some(parent) => format!("{parent} > {} > {}", note.title, anchor.heading),
                None => format!("{} > {}", note.title, anchor.heading),
            };
            for (part, window) in windows(&section, MAX_WINDOW_CHARS, OVERLAP_CHARS)
                .into_iter()
                .enumerate()
            {
                jobs.push(ChunkJob::new(
                    ChunkKey {
                        note_id: note.id.clone(),
                        scope: ChunkScope::Anchor,
                        anchor_id: anchor.anchor_id.clone(),
                        part: part as u32,
                    },
                    Some(anchor.line),
                    format!("{prefix}\n\n{window}"),
                ));
            }
        }
    }

    jobs
}

/// Split `text` into windows of at most `max` bytes, packing whole paragraphs
/// where possible. Consecutive windows share roughly `overlap` bytes of tail.
pub fn windows(text: &str, max: usize, overlap: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.len() <= max {
        return vec![text.to_string()];
    }

    let mut out = Vec::new();
    let mut current = String::new();

    for paragraph in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if paragraph.len() > max {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            out.extend(split_long(paragraph, max, overlap));
            continue;
        }

        if !current.is_empty() && current.len() + 2 + paragraph.len() > max {
            let carry = tail(&current, overlap);
            let carry = if carry.len() + 2 + paragraph.len() > max {
                String::new()
            } else {
                carry
            };
            out.push(std::mem::replace(&mut current, carry));
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(paragraph);
    }

    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn split_long(paragraph: &str, max: usize, overlap: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < paragraph.len() {
        let mut end = (start + max).min(paragraph.len());
        while !paragraph.is_char_boundary(end) {
            end -= 1;
        }
        let piece = paragraph[start..end].trim();
        if !piece.is_empty() {
            out.push(piece.to_string());
        }
        if end == paragraph.len() {
            break;
        }
        let mut next = end.saturating_sub(overlap).max(start + 1);
        while !paragraph.is_char_boundary(next) {
            next += 1;
        }
        start = next;
    }
    out
}

fn tail(text: &str, overlap: usize) -> String {
    if text.len() <= overlap {
        return text.to_string();
    }
    let mut start = text.len() - overlap;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let rest = &text[start..];
    match rest.find(char::is_whitespace) {
        Some(index) => rest[index..].trim_start().to_string(),
        None => rest.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, note_type: &str) -> NoteSource {
        NoteSource {
            id: id.to_string(),
            title: format!("Title {id}"),
            parent_title: None,
            note_type: note_type.to_string(),
            tags: vec!["topic".to_string()],
            authors: vec!["Doe".to_string()],
            anchors: Vec::new(),
            lines: Vec::new(),
        }
    }

    #[test]
    fn short_text_is_one_window() {
        assert_eq!(windows("hello world", 100, 10), vec!["hello world"]);
        assert!(windows("   ", 100, 10).is_empty());
    }

    #[test]
    fn paragraph_windows_respect_the_limit_and_keep_all_text() {
        let text = (0..10)
            .map(|i| format!("p{i} {}", "x".repeat(200)))
            .collect::<Vec<_>>()
            .join("\n\n");
        let out = windows(&text, 500, 50);
        assert!(out.len() > 1);
        assert!(out.iter().all(|w| w.len() <= 500));
        for i in 0..10 {
            assert!(out.iter().any(|w| w.contains(&format!("p{i} "))));
        }
    }

    #[test]
    fn long_paragraph_is_split_on_char_boundaries() {
        let text = "é".repeat(1200);
        let out = windows(&text, 1000, 100);
        assert!(out.len() >= 3);
        assert!(out.iter().all(|w| w.len() <= 1000));
    }

    #[test]
    fn author_notes_only_get_a_note_chunk() {
        let mut author = source("a_doe", "author");
        author.anchors.push(AnchorSource {
            anchor_id: "a_doe".to_string(),
            heading: "Doe".to_string(),
            line: 7,
        });
        author.lines.push((7, "# Doe".to_string()));
        author.lines.push((8, "Bio text".to_string()));

        let jobs = build_chunks(&[author]);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].key.scope, ChunkScope::Note);
        assert!(jobs[0].text.starts_with("Title a_doe\nType: author"));
    }

    #[test]
    fn anchor_sections_cover_lines_until_the_next_heading() {
        let mut note = source("alpha", "note");
        note.anchors = vec![
            AnchorSource {
                anchor_id: "alpha".to_string(),
                heading: "Alpha".to_string(),
                line: 1,
            },
            AnchorSource {
                anchor_id: "alpha-second".to_string(),
                heading: "Second".to_string(),
                line: 4,
            },
            AnchorSource {
                anchor_id: "alpha-empty".to_string(),
                heading: "Empty".to_string(),
                line: 6,
            },
        ];
        note.lines = vec![
            (1, "# Alpha".to_string()),
            (2, "first body".to_string()),
            (4, "## Second".to_string()),
            (5, "second body".to_string()),
        ];

        let jobs = build_chunks(&[note]);
        let anchors: Vec<&ChunkJob> = jobs
            .iter()
            .filter(|j| j.key.scope == ChunkScope::Anchor)
            .collect();
        assert_eq!(anchors.len(), 2, "empty section is skipped");
        assert_eq!(anchors[0].key.anchor_id, "alpha");
        assert!(anchors[0].text.contains("first body"));
        assert!(!anchors[0].text.contains("second body"));
        assert_eq!(anchors[0].line, Some(1));
        assert_eq!(anchors[1].line, Some(4));
    }

    #[test]
    fn parent_title_appears_in_note_and_section_chunks() {
        let mut note = source("alpha", "note");
        note.parent_title = Some("Book".to_string());
        note.anchors.push(AnchorSource {
            anchor_id: "alpha".to_string(),
            heading: "Alpha".to_string(),
            line: 1,
        });
        note.lines = vec![(1, "# Alpha".to_string()), (2, "body".to_string())];

        let jobs = build_chunks(&[note]);
        let note_chunk = jobs
            .iter()
            .find(|job| job.key.scope == ChunkScope::Note)
            .unwrap();
        assert!(note_chunk.text.contains("\nParent: Book"));
        let anchor = jobs
            .iter()
            .find(|job| job.key.scope == ChunkScope::Anchor)
            .unwrap();
        assert!(anchor.text.starts_with("Book > Title alpha > Alpha"));
    }

    #[test]
    fn hash_changes_with_text() {
        let a = ChunkJob::new(
            ChunkKey {
                note_id: "n".to_string(),
                scope: ChunkScope::Note,
                anchor_id: String::new(),
                part: 0,
            },
            None,
            "one".to_string(),
        );
        let b = ChunkJob::new(a.key.clone(), None, "two".to_string());
        assert_ne!(a.text_hash, b.text_hash);
    }
}
