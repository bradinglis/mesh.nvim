use std::fmt::Display;

// use pdf_oxide::AnnotationSubtype;
// use pdf_oxide::Destination;
// use pdf_oxide::OutlineItem;
// use pdf_oxide::PdfDocument;
// use pdf_oxide::api::Rect;
// use pdf_oxide::layout::SpatialCollectionFiltering;
// use pdf_oxide::layout::TextChar;
// use pdf_oxide::pipeline::MarkdownOutputConverter;
// use pdf_oxide::pipeline::OrderedTextSpan;
// use pdf_oxide::pipeline::OutputConverter;
// use pdf_oxide::pipeline::TextPipelineConfig;
use serde::Deserialize;
use std::cmp::Ordering;

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

impl Display for AnnotType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AnnotType::Quote => "quote",
            AnnotType::Emphasis => "emphasis",
            AnnotType::Point => "point",
        })
    }
}

pub fn get_annotations(path: &std::path::Path) -> crate::Result<MyOutlineItem> {
    // let doc = PdfDocument::open(path)?;

    let mut res = Vec::new();
    // for page_num in doc.page_indices() {
    //     let annots = doc.get_annotations(page_num)?;
    //     for annot in annots {
    //         if annot.subtype_enum == AnnotationSubtype::Highlight {
    //             let quote = if let Some(quads) = annot.quad_points {
    //                 let mut rects = Vec::new();
    //                 for [_, _, x1, y1, x0, y0, _, _] in quads {
    //                     rects.push(Rect::from_points(
    //                         x0 as f32, y0 as f32, x1 as f32, y1 as f32,
    //                     ))
    //                 }
    //                 let page_chars = doc.extract_chars(page_num)?;
    //                 let lines: Vec<Vec<TextChar>> = rects
    //                     .into_iter()
    //                     .map(|rect| {
    //                         let mut line = page_chars.filter_by_rect(
    //                             &rect,
    //                             pdf_oxide::layout::RectFilterMode::MinOverlap(0.5),
    //                         );
    //                         line.sort_by(|x, y| {
    //                             if x.bbox.x > y.bbox.x {
    //                                 Ordering::Greater
    //                             } else if x.bbox.x < y.bbox.x {
    //                                 Ordering::Less
    //                             } else {
    //                                 Ordering::Equal
    //                             }
    //                         });
    //                         line
    //                     })
    //                     .collect();
    //
    //                 let mut chars: Vec<TextChar> = Vec::new();
    //
    //                 for mut line in lines {
    //                     if let Some(last) = chars.last() {
    //                         if last.char == '-' {
    //                             chars.pop();
    //                         } else if last.char != ' ' {
    //                             if is_italic(last) || is_bold(last) {
    //                                 let mut c = line.first().unwrap().clone();
    //                                 c.char = ' ';
    //                                 chars.push(c);
    //                             } else {
    //                                 let mut c = last.clone();
    //                                 c.char = ' ';
    //                                 chars.push(c);
    //                             }
    //                         }
    //                     }
    //                     chars.append(&mut line);
    //                 }
    //                 let spans: Vec<Span> = chars
    //                     .chunk_by(|a, b| {
    //                         a.char == ' ' || b.char == ' ' || (is_plain(a) == is_plain(b))
    //                     })
    //                     .map(|x| {
    //                         let s: String = x.iter().map(|x| x.char).collect();
    //                         if is_italic(&x[0]) {
    //                             Span::Italic(s)
    //                         } else if is_bold(&x[0]) {
    //                             Span::Bold(s)
    //                         } else {
    //                             Span::Plain(s)
    //                         }
    //                     })
    //                     .collect();
    //                 spans_to_markdown(spans)
    //             } else {
    //                 "".to_string()
    //             };
    //
    //             let comment = match annot.contents {
    //                 Some(x) if x.starts_with("Q: ") => {
    //                     Comment::Question(x.trim_prefix("Q: ").to_string())
    //                 }
    //                 Some(x) if x.starts_with("T: ") => {
    //                     Comment::Thought(x.trim_prefix("T: ").to_string())
    //                 }
    //                 Some(x) if x.starts_with("C: ") => {
    //                     Comment::Caveat(x.trim_prefix("C: ").to_string())
    //                 }
    //                 Some(x) if !x.is_empty() => Comment::Summary(x),
    //                 _ => Comment::None,
    //             };
    //
    //             let quote_kind = if let Some(color) = annot.color {
    //                 let r = color[0];
    //                 let g = color[1];
    //                 let b = color[2];
    //
    //                 if r == 1.0 {
    //                     AnnotType::Quote
    //                 } else if b == 1.0 {
    //                     AnnotType::Point
    //                 } else {
    //                     AnnotType::Emphasis
    //                 }
    //             } else {
    //                 AnnotType::Emphasis
    //             };
    //
    //             res.push(Annotation {
    //                 comment,
    //                 page_num,
    //                 quote_kind,
    //                 quote,
    //             });
    //         }
    //     }
    // }

    Ok(
        // match doc.get_outline()? {
        // Some(outline) => MyOutlineItem {
        //     title: "Annotations Export".to_string(),
        //     page_start: 0,
        //     page_end: doc.page_count()?,
        //     annotations: Vec::new(),
        //     children: outline_handle(outline, &res, doc.page_count()?),
        // },
        // None =>
        MyOutlineItem {
            title: "Annotations Export".to_string(),
            page_start: 0,
            page_end: 0,
            annotations: res,
            children: Vec::new(),
        },
    )
}

// fn spans_to_markdown(input: Vec<Span>) -> String {
//     let mut spans: Vec<Span> = Vec::new();
//
//     for i in 0..input.len() {
//         let span = match input[i].clone() {
//             Span::Italic(italic) => Span::Italic(italic.trim().to_string()),
//             Span::Bold(bold) => Span::Bold(bold.trim().to_string()),
//             Span::Plain(plain) => {
//                 let mut res = String::new();
//                 if !spans.is_empty()
//                     && let Some(Span::Italic(previous)) | Some(Span::Bold(previous)) =
//                         spans.get(i - 1)
//                     && previous.ends_with(char::is_alphabetic)
//                     && plain.starts_with(char::is_alphabetic)
//                 {
//                     res.push(' ');
//                 }
//                 res.push_str(&plain);
//
//                 if let Some(Span::Italic(next)) | Some(Span::Bold(next)) = input.get(i + 1)
//                     && next.trim().starts_with(char::is_alphabetic)
//                     && plain.ends_with(char::is_alphabetic)
//                 {
//                     res.push(' ');
//                 }
//                 Span::Plain(res)
//             }
//         };
//         spans.push(span);
//     }
//
//     let res: String = spans
//         .into_iter()
//         .map(|span| match span {
//             Span::Italic(s) => format!("*{}*", s.trim()),
//             Span::Bold(s) => format!("**{}**", s.trim()),
//             Span::Plain(s) => s,
//         })
//         .collect();
//
//     res.replace("‘", "'")
//         .replace("’", "'")
//         .replace("“", "\"")
//         .replace("”", "\"")
//         .replace("\n\n", " ")
//         .replace("&dquo;", "\"")
//         .replace("dquo;", "\"")
//         .replace("&dquo", "\"")
//         .replace("[", "\\[")
//         .replace("]", "\\]")
//         .replace("ﬀ", "ff")
//         .replace("ﬁ", "fi")
//         .replace("ﬂ", "fl")
//         .replace("ﬃ", "ffi")
//         .replace("ﬄ", "ffl")
//         .replace("ﬅ", "st")
//         .replace("ﬆ", "st")
//         .split_whitespace()
//         .collect::<Vec<&str>>()
//         .join(" ")
// }

// fn outline_handle(
//     input: Vec<OutlineItem>,
//     annotations: &Vec<Annotation>,
//     parent_page: usize,
// ) -> Vec<MyOutlineItem> {
//     let mut res = Vec::new();
//     for i in 0..input.len() {
//         let section_end = if let Some(sibling) = input.get(i + 1)
//             && let Some(Destination::PageIndex(sibling_page)) = sibling.dest
//         {
//             sibling_page
//         } else {
//             parent_page
//         };
//
//         let next_page = if let Some(child) = input[i].children.first()
//             && let Some(Destination::PageIndex(child_page)) = child.dest
//         {
//             child_page
//         } else {
//             section_end
//         };
//         if let Some(Destination::PageIndex(start_index)) = input[i].dest {
//             let mut title = input[i]
//                 .title
//                 .clone()
//                 .replace("‘", "'")
//                 .replace("’", "'")
//                 .replace("“", "\"")
//                 .replace("”", "\"");
//
//             title.retain(|c| c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation());
//             title = title.split_whitespace().collect::<Vec<&str>>().join(" ");
//
//             let section_annots = get_section_annots(annotations, start_index, next_page);
//             if !section_annots.is_empty() {
//                 res.push(MyOutlineItem {
//                     title,
//                     page_start: start_index,
//                     page_end: section_end,
//                     annotations: get_section_annots(annotations, start_index, next_page),
//                     children: outline_handle(input[i].children.clone(), annotations, section_end),
//                 });
//             }
//         }
//     }
//     res
// }

// fn get_section_annots(input: &[Annotation], start: usize, end: usize) -> Vec<Annotation> {
//     input
//         .iter()
//         .filter(|x| x.page_num >= start && x.page_num < end)
//         .cloned()
//         .collect()
// }
//
// impl Display for Annotation {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let comment_text: Option<String> = match &self.comment {
//             Comment::Thought(s) => Some(
//                 format!("> [!THOUGHT] {}", s)
//                     .split("\n")
//                     .map(String::from)
//                     .collect(),
//             ),
//             Comment::Caveat(s) => Some(
//                 format!("> [!CAVEAT] {}", s)
//                     .split("\n")
//                     .map(String::from)
//                     .collect(),
//             ),
//             Comment::Question(s) => Some(
//                 format!("> [!QUESTION] {}", s)
//                     .split("\n")
//                     .map(String::from)
//                     .collect(),
//             ),
//             Comment::Summary(s) => Some(s.split("\n").map(String::from).collect()),
//             Comment::None => None,
//         };
//
//         let quote_text = match self.quote_kind {
//             AnnotType::Quote => {
//                 format!(
//                     "\n> [!QUOTE] {} {{p. {}}}\n",
//                     self.quote.trim(),
//                     self.page_num + 1
//                 )
//             }
//             AnnotType::Emphasis => {
//                 format!("- =={}==", self.quote.trim())
//             }
//             AnnotType::Point => {
//                 format!("- {}", self.quote.trim())
//             }
//         };
//         match comment_text {
//             Some(comment) => f.write_fmt(format_args!("\n\n{}\n{}\n", comment, quote_text)),
//             None => f.write_fmt(format_args!("{}\n", quote_text)),
//         }
//     }
// }
//
// fn annotations_to_string(input: &[Annotation]) -> String {
//     let s: String = input.iter().map(|x| x.to_string()).collect();
//     s.replace("\n\n\n", "\n\n")
// }
//
// pub fn display_outline_item(input: &MyOutlineItem, level: usize) -> String {
//     let prefix = "#".repeat(level);
//     if level >= 6 {
//         return "".to_string();
//     }
//
//     let children: String = if input.children.is_empty() {
//         "".to_string()
//     } else {
//         let s: String = input
//             .children
//             .iter()
//             .map(|x| display_outline_item(x, level + 1))
//             .collect();
//         format!("{}\n\n", s)
//     };
//
//     let notes: String = if input.annotations.is_empty() {
//         "".to_string()
//     } else {
//         format!("{}\n\n", annotations_to_string(&input.annotations))
//     };
//
//     if level == 1 {
//         format!("{} {}\n\n{}{}", prefix, input.title, notes, children.trim())
//             .replace("\n\n\n", "\n\n")
//     } else {
//         format!(
//             "{} {} {{p. {}-{}}}\n\n{}\n\n{}\n\n",
//             prefix,
//             input.title,
//             input.page_start,
//             input.page_end,
//             notes,
//             children.trim()
//         )
//     }
// }
//
// fn is_italic(c: &TextChar) -> bool {
//     c.is_italic || c.font_name.ends_with(".I")
// }
//
// fn is_bold(c: &TextChar) -> bool {
//     c.font_weight.is_bold() || c.font_name.ends_with(".B")
// }
//
// fn is_plain(c: &TextChar) -> bool {
//     !is_bold(c) && !is_italic(c)
// }
