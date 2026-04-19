use std::fmt::Display;
use unicode_bom::Bom;

use crate::Result;
use crate::Results;
use crate::trier;
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
    pub comment: Option<String>,
    pub page: u32,
    pub quote: String,
    pub priority: Priority,
}

#[derive(Debug)]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        })
    }
}

pub fn get_annotations(path: &std::path::Path) -> Results<Annotation> {
    let doc = lopdf::Document::load(path)?;

    let pages = doc.get_pages();

    pages
        .iter()
        .flat_map(|(page_num, page)| {
            doc.get_page_annotations(*page)
                .unwrap()
                .into_iter()
                .map(|x| {
                    let u2: Vec<u8> = x
                        .get(b"onyxtag")?
                        .as_str()?
                        .to_vec()
                        .iter()
                        .flat_map(|x| match *x {
                            // —
                            132 => vec![226, 128, 148],
                            // ‘
                            143 => vec![226, 128, 152],
                            // ’
                            144 => vec![226, 128, 153],
                            // ł
                            155 => vec![197, 130],
                            a => vec![a],
                        })
                        .collect();

                    let onyxtag = String::from_utf8_lossy(&u2);
                    let v: OnyxTag = serde_json::from_str(&onyxtag)?;

                    let comment = x.get(b"Contents")?.as_str()?;

                    let bom: Bom = comment.into();
                    let comment = &comment[bom.len()..];


                    let comment = match bom {
                        Bom::Utf8 => String::from_utf8(comment.to_vec()).unwrap_or_default(),
                        Bom::Utf16Be => String::from_utf16be(comment).unwrap_or_default(),
                        Bom::Utf16Le => String::from_utf16le(comment).unwrap_or_default(),
                        _ => trier::Tryer::new()
                        .or_try(|| String::from_utf8(comment.to_vec()))
                        .or_try(|| String::from_utf16be(comment))
                        .or_try(|| Some(String::from_utf8_lossy(comment)))
                        .unwrap_or_lazy(String::new),
                    };

                    let comment = if !comment.is_empty() {
                        Some(comment)
                    } else {
                        None
                    };

                    let extra: Highlight = serde_json::from_str(&v.extra_attr)?;
                    let quote = extra.quote;
                    let priority = match v.color.as_str() {
                        "4278251775" => Priority::Medium,
                        _ => Priority::Low,
                    };

                    Ok(Annotation {
                        comment,
                        page: *page_num,
                        quote,
                        priority,
                    })
                })
        })
        .collect()
}
