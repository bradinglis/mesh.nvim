use std::cmp::Ordering;

use pdf_oxide::layout::TextChar;

/// OCR text layers place each word in its own tight box, so a quad's vertical
/// band can miss a word whose baseline sits a fraction of a point outside it.
const VERTICAL_TOLERANCE: f32 = 0.75;

/// A highlight quad expressed as an axis-aligned range in PDF user space.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
}

impl Rect {
    /// Build the bounding box of a quad.
    ///
    /// A PDF quad is eight numbers (`x1 y1 x2 y2 x3 y3 x4 y4`); depending on
    /// the producer the points may be listed clockwise or counter-clockwise,
    /// so we take the extents rather than assuming a corner order.
    fn from_quad(quad: &[f64; 8]) -> Self {
        let mut xs = [f32::INFINITY, f32::NEG_INFINITY];
        let mut ys = [f32::INFINITY, f32::NEG_INFINITY];
        for pair in quad.chunks_exact(2) {
            let (x, y) = (pair[0] as f32, pair[1] as f32);
            xs[0] = xs[0].min(x);
            xs[1] = xs[1].max(x);
            ys[0] = ys[0].min(y);
            ys[1] = ys[1].max(y);
        }
        Self {
            x0: xs[0],
            x1: xs[1],
            y0: ys[0],
            y1: ys[1],
        }
    }

    fn contains_center(&self, c: &TextChar) -> bool {
        let cx = c.bbox.x + c.bbox.width / 2.0;
        let cy = c.bbox.y + c.bbox.height / 2.0;
        cx >= self.x0
            && cx <= self.x1
            && cy >= self.y0 - VERTICAL_TOLERANCE
            && cy <= self.y1 + VERTICAL_TOLERANCE
    }

    fn contains_vertical_center(&self, c: &TextChar) -> bool {
        let cy = c.bbox.y + c.bbox.height / 2.0;
        cy >= self.y0 - VERTICAL_TOLERANCE && cy <= self.y1 + VERTICAL_TOLERANCE
    }
}

pub fn get_annotations(path: &std::path::Path) -> Result<Vec<String>, pdf_oxide::Error> {
    let doc = pdf_oxide::PdfDocument::open(path)?;

    let mut res = Vec::new();
    for page_num in doc.page_indices() {
        let page = Page::init(&doc, page_num)?;

        for annot in doc.get_annotations(page_num)? {
            if let Some(text) = page.get_highlight(annot)? {
                res.push(text);
            }
        }
    }

    Ok(res)
}

struct Page {
    chars: Vec<TextChar>,
    body_font: String,
}

impl Page {
    pub fn init(doc: &pdf_oxide::PdfDocument, page_num: usize) -> Result<Page, pdf_oxide::Error> {
        let chars = doc.extract_chars(page_num)?;
        let body_font = dominant_font(&chars);
        Ok(Page {
            chars,
            body_font,
        })
    }
    pub fn get_highlight(&self, annot: pdf_oxide::Annotation) -> Result<Option<String>, pdf_oxide::Error> {
        if annot.subtype_enum != pdf_oxide::AnnotationSubtype::Highlight {
            return Ok(None)
        }

        let mut lines = Vec::new();
        if let Some(quads) = &annot.quad_points {
            let mut rects: Vec<Rect> = quads.iter().map(Rect::from_quad).collect();
            // Quads are usually stored top-to-bottom, but not all producers
            // guarantee it. PDF user space has y growing upwards.
            rects.sort_by(|a, b| b.y1.partial_cmp(&a.y1).unwrap_or(Ordering::Equal));
            for rect in rects {
                let line = chars_in_rect(&self.chars, rect);
                if !line.is_empty() {
                    lines.push(line);
                }
            }
        }
        Ok(Some(lines_to_markdown(&lines, &self.body_font)))
    }
}

/// Font used for most alphabetic characters on the page.
///
/// Fonts in PDFs produced by print pipelines are often subsetted with
/// obfuscated names, so this is a more reliable "body text" baseline than
/// looking for words like `Italic` in the name.
fn dominant_font(chars: &[TextChar]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for c in chars.iter().filter(|c| c.char.is_alphanumeric()) {
        match counts.iter_mut().find(|(name, _)| *name == c.font_name) {
            Some((_, count)) => *count += 1,
            None => counts.push((c.font_name.as_str(), 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.to_string())
        .unwrap_or_default()
}

/// Characters belonging to one highlight quad, left to right.
fn chars_in_rect(chars: &[TextChar], rect: Rect) -> Vec<TextChar> {
    // Restrict to the quad's vertical band first so the trailing-punctuation
    // check below cannot pick up a glyph from an adjacent line.
    let mut candidates: Vec<&TextChar> = chars
        .iter()
        .filter(|c| rect.contains_vertical_center(c))
        .collect();
    candidates.sort_by(|a, b| a.bbox.x.partial_cmp(&b.bbox.x).unwrap_or(Ordering::Equal));

    let mut selected: Vec<(usize, TextChar)> = Vec::new();
    for (index, c) in candidates.iter().enumerate() {
        // On the right, the quad frequently stops short of the final glyph's
        // advance, so use the glyph centre rather than its right edge.
        if rect.contains_center(c) {
            selected.push((index, (*c).clone()));
        }
    }

    // Quads are often anchored a fraction of a point inside the first selected
    // glyph. If that glyph is in the middle of a word, the crop picked up a
    // fragment of the preceding word (e.g. `f. ` before `The`); skip forward to
    // the next word boundary instead of emitting the fragment.
    let mut start = 0;
    if let Some(&(index, _)) = selected.first()
        && index > 0
        && !candidates[index - 1].char.is_whitespace()
        && candidates[index].bbox.x
            - (candidates[index - 1].bbox.x + candidates[index - 1].bbox.width)
            < 0.2
    {
        while start < selected.len() {
            let (index, _) = selected[start];
            if index == 0 {
                break;
            }
            let current = candidates[index];
            let previous = candidates[index - 1];
            let adjacent = current.bbox.x - (previous.bbox.x + previous.bbox.width) < 0.2;
            if previous.char.is_whitespace() || !adjacent {
                break;
            }
            start += 1;
        }
    }

    let mut line: Vec<TextChar> = selected[start..]
        .iter()
        .map(|(_, c)| c.clone())
        .collect();

    // Highlight quads often stop at the *origin* of the final glyph rather
    // than its advance, which leaves an adjacent punctuation mark (e.g. a
    // closing `?` or `.`) just outside the rectangle. Pull in a single
    // trailing punctuation glyph that is flush against the crop edge.
    if let Some(last) = line.last() {
        let right = last.bbox.x + last.bbox.width;
        if let Some(next) = candidates.iter().find(|c| c.bbox.x >= right - 0.01) {
            if next.bbox.x - right < 1.0 && is_punctuation(next.char) {
                line.push((*next).clone());
            }
        }
    }

    // Drop whitespace at both edges introduced by the crop.
    let start = line.iter().position(|c| !c.char.is_whitespace());
    let end = line.iter().rposition(|c| !c.char.is_whitespace());
    match (start, end) {
        (Some(start), Some(end)) => line[start..=end].to_vec(),
        _ => Vec::new(),
    }
}

fn is_punctuation(ch: char) -> bool {
    !ch.is_alphanumeric() && !ch.is_whitespace()
}

fn is_emphasis(c: &TextChar, line_has_body_font: bool, body_font: &str) -> bool {
    if c.is_italic {
        return true;
    }
    // A font change within an otherwise body-text line is the only signal
    // available in some PDFs (subsetted fonts carry no italic flag or usable
    // name), so treat an alphabetic run in a different font as emphasis.
    line_has_body_font && c.char.is_alphanumeric() && c.font_name != body_font
}

/// Concatenate the extracted lines, undoing end-of-line hyphenation and
/// emitting Markdown emphasis for italic runs.
fn lines_to_markdown(lines: &[Vec<TextChar>], body_font: &str) -> String {
    let mut out: Vec<(char, bool)> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }

        let line_has_body_font = line
            .iter()
            .any(|c| c.char.is_alphanumeric() && c.font_name == body_font);

        if index > 0 && !out.is_empty() {
            if matches!(out.last(), Some((ch, _)) if *ch == '-' || *ch == '\u{2010}') {
                // A hyphen at the end of a line is a hyphenation break: remove
                // it and join the fragments directly.
                out.pop();
            } else if let Some(first) = line.first()
                && !first.char.is_whitespace()
                && !out.last().is_some_and(|(ch, _)| ch.is_whitespace())
            {
                out.push((' ', false));
            }
        }

        let mut previous: Option<&TextChar> = None;
        for c in line {
            // Word gaps are sometimes encoded as text-positioning offsets
            // rather than space glyphs (common around font changes), so
            // synthesise the missing space from the geometric gap.
            if let Some(previous) = previous {
                let gap = c.bbox.x - (previous.bbox.x + previous.bbox.width);
                let em = previous.font_size.max(c.font_size);
                if gap > 0.25 * em && !previous.char.is_whitespace() && !c.char.is_whitespace() {
                    out.push((' ', false));
                }
            }
            out.push((c.char, is_emphasis(c, line_has_body_font, body_font)));
            previous = Some(c);
        }
    }

    let mut markdown = String::new();
    let mut index = 0;
    while index < out.len() {
        if !out[index].1 {
            markdown.push(out[index].0);
            index += 1;
            continue;
        }

        // Collect one italic run. Whitespace at its edges is moved outside the
        // emphasis markers so that a space glyph tagged italic by the producer
        // does not produce `word* next*` instead of `word *next*`.
        let mut end = index;
        while end < out.len() && out[end].1 {
            end += 1;
        }
        let run = &out[index..end];
        let first = run.iter().position(|(ch, _)| !ch.is_whitespace());
        let last = run.iter().rposition(|(ch, _)| !ch.is_whitespace());
        match (first, last) {
            (Some(first), Some(last)) => {
                for (ch, _) in &run[..first] {
                    markdown.push(*ch);
                }
                markdown.push('*');
                for (ch, _) in &run[first..=last] {
                    markdown.push(*ch);
                }
                markdown.push('*');
                for (ch, _) in &run[last + 1..] {
                    markdown.push(*ch);
                }
            }
            _ => {
                for (ch, _) in run {
                    markdown.push(*ch);
                }
            }
        }
        index = end;
    }
    decode_entities(markdown)
}

/// Decode the handful of named entities that OCR text layers emit in place of
/// the glyphs they could not map. `&dquo;` is the most common: it is the SGML
/// name for a double quote, not a valid HTML entity.
fn decode_entities(text: String) -> String {
    [
        ("&dquo;", "\""),
        ("&ldquo;", "\""),
        ("&rdquo;", "\""),
        ("&lsquo;", "'"),
        ("&rsquo;", "'"),
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("‘", "'"),
        ("’", "'")
    ]
    .into_iter()
    .fold(text, |text, (entity, replacement)| {
        text.replace(entity, replacement)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // `test_one` exercises rectangle cropping, Markdown emphasis and typographic
    // quotes. `test_two` exercises multi-line cropping and end-of-line
    // hyphenation. Its fixture is a PDFium re-render in which the entire body
    // text uses a single non-italic font, so the source article's italics
    // (`machinery`, `technology`) are not recoverable from the PDF; the
    // expectation therefore asserts the faithful extraction.
    #[test]
    fn test_one() -> Result<(), pdf_oxide::Error> {
        get_annotations(std::path::Path::new("./tests/quote_and_markdown_test.pdf")).map(|x| {
            assert_eq!(
                x,
                vec!["technological determinism can be interpreted as the key ingredient of an ideology that the *Capital* introduced as the 'fetishism of commodities'."]
            )
        })
    }

    #[test]
    fn test_two() -> Result<(), pdf_oxide::Error> {
        get_annotations(std::path::Path::new("./tests/test_2.pdf")).map(|x| assert_eq!(x, vec!["His unequivocal answer was: machinery. \"It is the Age of Machinery,\" he wrote, \"in every outward and inward sense of that word.\"5 During the next half century, however, machinery - like the alternatives just mentioned - turned out to be unsuitable. But why? Why did technology prove to be preferable?"]))
    }

    // `test_three` has a real text layer with Times-Roman and Times-Italic
    // fonts, so the source article's emphasis is recoverable.
    #[test]
    fn test_three() -> Result<(), pdf_oxide::Error> {
        get_annotations(std::path::Path::new("./tests/test_3.pdf")).map(|x| 
            assert_eq!(
                x, 
                vec![
                    "The German idea of utility was not at all *subjective* because of the socially accepted superior position of *Polizei* over *Oekonomie.*",
                    "the Linnaean description of \"use-values\" does not have this feature and therefore cannot be identified as the basis of modern economics: instead, this kind of description, because of its *scientific* and *intersubjective* character, can be easily identified as the basis of the modern concept of *technology.*",
                    "*Technologie,* that is to a description of productive phenomena which has *no precedent* in the history of world scientific speculation."
                ]

            ))
    }
    // `test_four` is a scanned page: the visible article is a CCITT image and
    // the only text layer is an invisible OCR approximation. Its italics exist
    // only as pixels (the PDF contains no italic font), the OCR layer says
    // `graspea` and omits the closing quote after `itself.`, and the quads are
    // slightly offset (they begin inside `For` and include the comma after
    // `Ideology`). This expectation therefore asserts the faithful OCR-layer
    // extraction rather than the visually corrected article text.
    #[test]
    fn test_four() -> Result<(), pdf_oxide::Error> {
        get_annotations(std::path::Path::new("./tests/test_4.pdf")).map(|x| 
            assert_eq!(
                x, 
                vec![
                    "the commodity has been graspea in such a way that it posits itself as money in the process of an \"immanent going beyond itself.",
                    "For these interpretations it is consequently the \"secret\" of the magnitude of value, not the \"secret\" of this \"objective semblance\" or of the thing-like form which constitutes the \"mystical character\" of the commodity.",
                    "commodity fetishism which misses its essence can be thus characterised: the authors refer to some sentences from the fetishism chapter of Capital and interpret them, conceptually and also for the most part terminologically, in the manner of the German Ideology,",
                ]

            ))
    }
}
