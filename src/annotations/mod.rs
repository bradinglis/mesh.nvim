use std::fmt::Display;

use pdf_oxide::Destination;
use pdf_oxide::OutlineItem;
use pdf_oxide::PdfDocument;
use pdf_oxide::layout::TextChar;

pub(crate) mod highlights;

#[derive(Debug)]
pub struct MyOutlineItem {
    pub title: String,
    pub page_start: usize,
    pub page_end: usize,
    pub annotations: Vec<Annotation>,
    pub children: Vec<MyOutlineItem>,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub comment: Comment,
    pub page_num: usize,
    pub quote_kind: AnnotType,
    pub quote: String,
}

#[derive(Debug, Clone)]
pub enum Comment {
    Thought(String),
    Caveat(String),
    Question(String),
    Summary(String),
    None,
}

#[derive(Debug, Clone)]
pub enum AnnotType {
    Quote,
    Emphasis,
    Point,
}

struct Page {
    chars: Vec<TextChar>,
    body_font: String,
}

impl Page {
    pub fn init(doc: &pdf_oxide::PdfDocument, page_num: usize) -> Result<Page, pdf_oxide::Error> {
        let chars = doc.extract_chars(page_num)?;
        let body_font = highlights::dominant_font(&chars);
        Ok(Page { chars, body_font })
    }
}

pub fn get_annotations(path: &std::path::Path) -> crate::Result<MyOutlineItem> {
    let doc = PdfDocument::open(path)?;

    let mut res = Vec::new();
    for page_num in doc.page_indices() {
        let page = Page::init(&doc, page_num)?;

        for annot in doc.get_annotations(page_num)? {
            if annot.subtype_enum != pdf_oxide::AnnotationSubtype::Highlight {
                continue;
            }
            let Some(quote) = page.get_highlight(&annot)? else {
                continue;
            };
            if quote.trim().is_empty() {
                continue;
            }

            let comment = match annot.contents {
                Some(x) if x.starts_with("Q: ") => {
                    Comment::Question(x.trim_prefix("Q: ").to_string())
                }
                Some(x) if x.starts_with("T: ") => {
                    Comment::Thought(x.trim_prefix("T: ").to_string())
                }
                Some(x) if x.starts_with("C: ") => {
                    Comment::Caveat(x.trim_prefix("C: ").to_string())
                }
                Some(x) if !x.is_empty() => Comment::Summary(x),
                _ => Comment::None,
            };

            let quote_kind = if let Some(color) = annot.color {
                let r = color[0];
                let b = color[2];
                if r == 1.0 {
                    AnnotType::Quote
                } else if b == 1.0 {
                    AnnotType::Point
                } else {
                    AnnotType::Emphasis
                }
            } else {
                AnnotType::Emphasis
            };

            res.push(Annotation {
                comment,
                page_num,
                quote_kind,
                quote,
            });
        }
    }

    Ok(match doc.get_outline()? {
        Some(outline) => MyOutlineItem {
            title: "Annotations Export".to_string(),
            page_start: 0,
            page_end: doc.page_count()?,
            annotations: Vec::new(),
            children: outline_handle(outline, &res, doc.page_count()?),
        },
        None => MyOutlineItem {
            title: "Annotations Export".to_string(),
            page_start: 0,
            page_end: doc.page_count()?,
            annotations: res,
            children: Vec::new(),
        },
    })
}

fn outline_handle(
    input: Vec<OutlineItem>,
    annotations: &[Annotation],
    parent_page: usize,
) -> Vec<MyOutlineItem> {
    let mut res = Vec::new();
    let mut items = input.into_iter().peekable();

    while let Some(item) = items.next() {
        let section_end = items
            .peek()
            .and_then(|sibling| match sibling.dest {
                Some(Destination::PageIndex(page)) => Some(page),
                _ => None,
            })
            .unwrap_or(parent_page);

        let next_page = item
            .children
            .first()
            .and_then(|child| match child.dest {
                Some(Destination::PageIndex(page)) => Some(page),
                _ => None,
            })
            .unwrap_or(section_end);

        let Some(Destination::PageIndex(start_index)) = item.dest else {
            continue;
        };

        let section_annots = get_section_annots(annotations, start_index, next_page);
        if section_annots.is_empty() {
            continue;
        }

        let mut title = item
            .title
            .replace(['‘', '’'], "'")
            .replace(['“', '”'], "\"");
        title.retain(|c| c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation());
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");

        res.push(MyOutlineItem {
            title,
            page_start: start_index,
            page_end: section_end,
            annotations: section_annots,
            children: outline_handle(item.children, annotations, section_end),
        });
    }

    res
}

fn get_section_annots(input: &[Annotation], start: usize, end: usize) -> Vec<Annotation> {
    input
        .iter()
        .filter(|x| x.page_num >= start && x.page_num < end)
        .cloned()
        .collect()
}

impl Display for Annotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let comment_text = match &self.comment {
            Comment::Thought(s) => Some(format!("> [!THOUGHT] {}", s.replace('\n', ""))),
            Comment::Caveat(s) => Some(format!("> [!CAVEAT] {}", s.replace('\n', ""))),
            Comment::Question(s) => Some(format!("> [!QUESTION] {}", s.replace('\n', ""))),
            Comment::Summary(s) => Some(s.replace('\n', "")),
            Comment::None => None,
        };

        let quote_text = match self.quote_kind {
            AnnotType::Quote => format!(
                "\n> [!QUOTE] {} {{p. {}}}\n",
                self.quote.trim(),
                self.page_num + 1
            ),
            AnnotType::Emphasis => format!("- =={}==", self.quote.trim()),
            AnnotType::Point => format!("- {}", self.quote.trim()),
        };

        match comment_text {
            Some(comment) => write!(f, "\n\n{comment}\n{quote_text}\n"),
            None => writeln!(f, "{quote_text}"),
        }
    }
}

fn annotations_to_string(input: &[Annotation]) -> String {
    let s: String = input.iter().map(|x| x.to_string()).collect();
    s.replace("\n\n\n", "\n\n")
}

pub fn display_outline_item(input: &MyOutlineItem, level: usize) -> String {
    if level >= 6 {
        return String::new();
    }

    let prefix = "#".repeat(level);
    let children = if input.children.is_empty() {
        String::new()
    } else {
        let s: String = input
            .children
            .iter()
            .map(|x| display_outline_item(x, level + 1))
            .collect();
        format!("{}\n\n", s)
    };

    let notes = if input.annotations.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", annotations_to_string(&input.annotations))
    };

    if level == 1 {
        format!("{} {}\n\n{}{}", prefix, input.title, notes, children.trim())
            .replace("\n\n\n", "\n\n")
    } else {
        format!(
            "{} {} {{p. {}-{}}}\n\n{}\n\n{}\n\n",
            prefix,
            input.title,
            input.page_start,
            input.page_end,
            notes,
            children.trim()
        )
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;

    #[test]
    fn displays_top_level_title() {
        let item = MyOutlineItem {
            title: "Annotations Export".to_string(),
            page_start: 0,
            page_end: 1,
            annotations: Vec::new(),
            children: Vec::new(),
        };
        let out = display_outline_item(&item, 1);
        assert!(out.contains("# Annotations Export"));
    }
}
