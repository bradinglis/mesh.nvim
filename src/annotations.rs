use std::default;

use pdf_oxide::pipeline::OutputConverter;

pub fn get_annotations(path: &std::path::Path) -> Result<String, pdf_oxide::Error> {
    let doc = pdf_oxide::PdfDocument::open(path)?;

    let mut res = Vec::new();
    for page_num in doc.page_indices() {
        let annots = doc.get_annotations(page_num)?;
        for annot in annots {
            if annot.subtype_enum == pdf_oxide::AnnotationSubtype::Highlight {
                let quote = if let Some(quads) = annot.quad_points {
                    let mut rects = Vec::new();
                    for [_, _, x1, y1, x0, y0, _, _] in quads {
                        let rect = pdf_oxide::api::Rect::from_points(
                            x0 as f32, y0 as f32, x1 as f32, y1 as f32,
                        );
                        rects.push(rect);
                    }
                    let mut spans = Vec::new();
                    let mut text = Vec::new();
                    for rect in rects {
                        text.push(doc.extract_text_in_rect(
                            page_num,
                            rect,
                            pdf_oxide::layout::RectFilterMode::MinOverlap(0.5),
                        )?);
                        spans.append(&mut doc.extract_spans_in_rect(
                            page_num,
                            rect,
                            pdf_oxide::layout::RectFilterMode::MinOverlap(0.5),
                        )?);
                    }
                    print!("{:#?}", spans);
                    print!("{:#?}", text);
                    let pipeline = pdf_oxide::pipeline::TextPipeline::new();
                    let ord = pipeline.process(spans, pdf_oxide::pipeline::ReadingOrderContext::new())?;
                    let conv = pdf_oxide::pipeline::MarkdownOutputConverter::new();
                    conv.convert(&ord, &pdf_oxide::pipeline::TextPipelineConfig::default())?
                } else {
                    "".to_string()
                };

                res.push(quote);
            }
        }
    }
    Ok(res.iter().map(|s| format!("{}", s)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both tests are failing, the first with getting the quotes (which doc.extract_text_in_rect is
    // successful in reading) and both with correct rectangle boundaries and italics
    // parsing/rendering.
    #[test]
    fn test_one() -> Result<(), pdf_oxide::Error> {
        get_annotations(std::path::Path::new("./tests/quote_and_markdown_test.pdf")).map(|x| {
            assert_eq!(
                x,
                "technological determinism can be interpreted as the key ingredient of an ideology that the *Capital* introduced as the ‘fetishism of commodities’."
            )
        })
    }

    #[test]
    fn test_two() -> Result<(), pdf_oxide::Error> {
        get_annotations(std::path::Path::new("./tests/test_2.pdf")).map(|x| assert_eq!(x, "His unequivocal answer was: *machinery*. \"It is the Age of Machinery,\" he wrote, \"in every outward and inward sense of that word.\"5 During the next half century, however, *machinery* - like the alternatives just mentioned - turned out to be unsuitable. But why? Why did *technology* prove to be preferable?")) } }
