use std::fmt::Display;
use unicode_bom::Bom;

use crate::MyError;
use crate::Results;
use crate::trier;
use pdf_oxide::AnnotationColor;
use pdf_oxide::PdfDocument;
use pdf_oxide::AnnotationSubtype;
use pdf_oxide::api::Rect;
use pdf_oxide::converters::MarkdownConverter;
use pdf_oxide::layout::TextSpan;
use pdf_oxide::pipeline::MarkdownOutputConverter;
use pdf_oxide::pipeline::OrderedTextSpan;
use pdf_oxide::pipeline::OutputConverter;
use pdf_oxide::pipeline::ReadingOrderContext;
use pdf_oxide::pipeline::TextPipeline;
use pdf_oxide::pipeline::TextPipelineConfig;
use std::cmp::Ordering;
use serde::Deserialize;

#[derive(Deserialize)]
struct OnyxTag {
    color: String,
    extra_attr: String,
}

#[derive(Deserialize)]
struct Highlight {
    quote: String,
}

#[derive(Debug)]
pub struct Annotation {
    pub comment: Comment,
    pub page_num: usize,
    pub quote_kind: AnnotType,
    pub quote: String,
}

#[derive(Debug)]
pub enum Comment {
    Thought(String),
    Caveat(String),
    Question(String),
    Summary(String),
    None,
}

#[derive(Debug)]
pub enum AnnotType {
    Quote,
    Emphasis,
    Point,
}

impl Display for AnnotType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AnnotType::Quote => "quote",
            AnnotType::Emphasis => "emphasis",
            AnnotType::Point => "point",
        })
    }
}

pub fn get_annotations(path: &std::path::Path) -> crate::Result<Vec<Annotation>> {
    let doc = PdfDocument::open(path)?;
    let outline = doc.get_outline()?;

    let config = TextPipelineConfig::default();
    let converter = MarkdownOutputConverter::new();
    let mut res = Vec::new();
    for page_num in doc.page_indices() {
        let annots = doc.get_annotations(page_num)?;
        for annot in annots {
            if annot.subtype_enum == AnnotationSubtype::Highlight {
                let quote = if let Some(quads) = annot.quad_points {
                    let mut reading_order: usize = 0;
                    let spans: Vec<OrderedTextSpan> = quads
                        .into_iter()
                        .flat_map(|quad| {
                            let r = Rect::from_points(
                                quad[4] as f32,
                                quad[5] as f32,
                                quad[2] as f32,
                                quad[3] as f32,
                            );

                            let mut spans = doc
                                .extract_spans_in_rect(
                                    page_num,
                                    r,
                                    pdf_oxide::layout::RectFilterMode::MinOverlap(0.5),
                                )
                                .unwrap();

                            spans.sort_unstable_by(|x, y| {
                                if x.bbox.x > y.bbox.x {
                                    Ordering::Greater
                                } else {
                                    Ordering::Less
                                }
                            });
                            let x = spans.into_iter().map(move |x| {
                                let x = OrderedTextSpan::new(x, reading_order);
                                reading_order += 1;
                                x
                            });
                            reading_order += x.len();
                            x
                        })
                        .collect();

                    converter
                        .convert(&spans, &config)?
                        .replace("‘", "'")
                        .replace("’", "'")
                        .replace("“", "\"")
                        .replace("”", "\"")
                        .replace("\n\n", " ")
                        .replace("&dquo;", "\"")
                } else {
                    "".to_string()
                };

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
                    Some(x) => Comment::Summary(x),
                    None => Comment::None,
                };

                let quote_kind = if let Some(color) = annot.color {
                    let r = color[0];
                    let g = color[1];
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
    }

    Ok(res)
}

