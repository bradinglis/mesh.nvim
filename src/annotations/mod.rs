use std::fmt::Display;
use std::fmt::format;

use pdf_oxide::AnnotationSubtype;
use pdf_oxide::Destination;
use pdf_oxide::OutlineItem;
use pdf_oxide::PdfDocument;
use pdf_oxide::api::Rect;
use pdf_oxide::layout::SpatialCollectionFiltering;
use pdf_oxide::layout::TextChar;
use pdf_oxide::pipeline::MarkdownOutputConverter;
use pdf_oxide::pipeline::OrderedTextSpan;
use pdf_oxide::pipeline::OutputConverter;
use pdf_oxide::pipeline::TextPipeline;
use pdf_oxide::pipeline::TextPipelineConfig;
use serde::Deserialize;
use std::cmp::Ordering;

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
pub enum Span {
    Italic(String),
    Bold(String),
    Plain(String),
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
    annotations: Vec<Annotation>,
}

impl Page {
    pub fn init(doc: &pdf_oxide::PdfDocument, page_num: usize) -> Result<Page, pdf_oxide::Error> {
        let chars = doc.extract_chars(page_num)?;
        let body_font = highlights::dominant_font(&chars);
        Ok(Page {
            chars,
            body_font,
        })
    }
    pub fn get_annotation(&self, annot: pdf_oxide:annotations::Annotation) -> Result<Annotation, pdf_oxide::Error> {
        let chars = doc.extract_chars(page_num)?;
        let body_font = dominant_font(&chars);
        Ok(Page {
            chars,
            body_font,
        })
    }
}

fn get_annotations(path: &std::path::Path) -> Result<Vec<String>, pdf_oxide::Error> {
    let doc = pdf_oxide::PdfDocument::open(path)?;

    let mut res = Vec::new();
    for page_num in doc.page_indices() {
        let page = Page::init(&doc, page_num)?;
        for annot in doc.get_annotations(page_num)? {
            let a = page.get_annotation(annot)? 
        }
    }

    Ok(res)
}

pub fn get_annotations(path: &std::path::Path) -> crate::Result<MyOutlineItem> {
    let doc = PdfDocument::open(path)?;

    let mut res = Vec::new();
    for page_num in doc.page_indices() {
        let page = Page::init(&doc, page_num)?;

        for annot in doc.get_annotations(page_num)? {
            if let Some(text) = page.get_highlight(annot)? {
                res.push(text);
            }
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

fn spans_to_markdown(input: Vec<Span>) -> String {
    let mut spans: Vec<Span> = Vec::new();

    for i in 0..input.len() {
        let span = match input[i].clone() {
            Span::Italic(italic) => Span::Italic(italic.trim().to_string()),
            Span::Bold(bold) => Span::Bold(bold.trim().to_string()),
            Span::Plain(plain) => {
                let mut res = String::new();
                if !spans.is_empty()
                    && let Some(Span::Italic(previous)) | Some(Span::Bold(previous)) =
                        spans.get(i - 1)
                    && previous.ends_with(char::is_alphabetic)
                    && plain.starts_with(char::is_alphabetic)
                {
                    res.push(' ');
                }
                res.push_str(&plain);

                if let Some(Span::Italic(next)) | Some(Span::Bold(next)) = input.get(i + 1)
                    && next.starts_with(char::is_whitespace)
                    && !plain.ends_with(char::is_whitespace)
                {
                    res.push(' ');
                }
                Span::Plain(res)
            }
        };
        spans.push(span);
    }

    let res: String = spans
        .into_iter()
        .map(|span| match span {
            Span::Italic(s) => format!("*{}*", s.trim()),
            Span::Bold(s) => format!("**{}**", s.trim()),
            Span::Plain(s) => s,
        })
        .collect();

    res.replace("‘", "'")
        .replace("’", "'")
        .replace("“", "\"")
        .replace("”", "\"")
        .replace("\n\n", " ")
        .replace("&dquo;", "\"")
        .replace("dquo;", "\"")
        .replace("&dquo", "\"")
        .replace("[", "\\[")
        .replace("]", "\\]")
        .replace("ﬀ", "ff")
        .replace("ﬁ", "fi")
        .replace("ﬂ", "fl")
        .replace("ﬃ", "ffi")
        .replace("ﬄ", "ffl")
        .replace("ﬅ", "st")
        .replace("ﬆ", "st")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn outline_handle(
    input: Vec<OutlineItem>,
    annotations: &Vec<Annotation>,
    parent_page: usize,
) -> Vec<MyOutlineItem> {
    let mut res = Vec::new();
    for i in 0..input.len() {
        let section_end = if let Some(sibling) = input.get(i + 1)
            && let Some(Destination::PageIndex(sibling_page)) = sibling.dest
        {
            sibling_page
        } else {
            parent_page
        };

        let next_page = if let Some(child) = input[i].children.first()
            && let Some(Destination::PageIndex(child_page)) = child.dest
        {
            child_page
        } else {
            section_end
        };
        if let Some(Destination::PageIndex(start_index)) = input[i].dest {
            let mut title = input[i]
                .title
                .clone()
                .replace("‘", "'")
                .replace("’", "'")
                .replace("“", "\"")
                .replace("”", "\"");

            title.retain(|c| c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation());
            title = title.split_whitespace().collect::<Vec<&str>>().join(" ");

            let section_annots = get_section_annots(annotations, start_index, next_page);
            if !section_annots.is_empty() {
                res.push(MyOutlineItem {
                    title,
                    page_start: start_index,
                    page_end: section_end,
                    annotations: get_section_annots(annotations, start_index, next_page),
                    children: outline_handle(input[i].children.clone(), annotations, section_end),
                });
            }
        }
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
        let comment_text: Option<String> = match &self.comment {
            Comment::Thought(s) => Some(
                format!("> [!THOUGHT] {}", s)
                    .split("\n")
                    .map(String::from)
                    .collect(),
            ),
            Comment::Caveat(s) => Some(
                format!("> [!CAVEAT] {}", s)
                    .split("\n")
                    .map(String::from)
                    .collect(),
            ),
            Comment::Question(s) => Some(
                format!("> [!QUESTION] {}", s)
                    .split("\n")
                    .map(String::from)
                    .collect(),
            ),
            Comment::Summary(s) => Some(s.split("\n").map(String::from).collect()),
            Comment::None => None,
        };

        let quote_text = match self.quote_kind {
            AnnotType::Quote => {
                format!(
                    "\n> [!QUOTE] {} {{p. {}}}\n",
                    self.quote.trim(),
                    self.page_num + 1
                )
            }
            AnnotType::Emphasis => {
                format!("- =={}==", self.quote.trim())
            }
            AnnotType::Point => {
                format!("- {}", self.quote.trim())
            }
        };
        match comment_text {
            Some(comment) => f.write_fmt(format_args!("\n\n{}\n{}\n", comment, quote_text)),
            None => f.write_fmt(format_args!("{}\n", quote_text)),
        }
    }
}

fn annotations_to_string(input: &[Annotation]) -> String {
    let s: String = input.iter().map(|x| x.to_string()).collect();
    s.replace("\n\n\n", "\n\n")
}

pub fn display_outline_item(input: &MyOutlineItem, level: usize) -> String {
    let prefix = "#".repeat(level);
    if level >= 6 {
        return "".to_string();
    }

    let children: String = if input.children.is_empty() {
        "".to_string()
    } else {
        let s: String = input
            .children
            .iter()
            .map(|x| display_outline_item(x, level + 1))
            .collect();
        format!("{}\n\n", s)
    };

    let notes: String = if input.annotations.is_empty() {
        "".to_string()
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

fn is_italic(c: &TextChar) -> bool {
    c.is_italic || c.font_name.ends_with(".I")
}

fn is_bold(c: &TextChar) -> bool {
    c.font_weight.is_bold() || c.font_name.ends_with(".B")
}

fn is_plain(c: &TextChar) -> bool {
    !is_bold(c) && !is_italic(c)
}

