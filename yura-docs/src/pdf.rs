//! PDF text extraction.
//!
//! The work is done by `pdf-extract`, which parses the object graph with
//! `lopdf` and then walks each page's content stream, decoding `Tj`/`TJ`
//! operators with the active font's encoding. That is the only way to get real
//! text out of a PDF without shelling out to poppler or mutool, both of which
//! would make the Windows cross-build depend on C libraries.
//!
//! Two cases deserve their own error because "not a readable PDF" would be a
//! useless thing to tell someone:
//!
//! * **encrypted** — a password-protected PDF parses fine but yields no text;
//!   we ask `lopdf` up front so the UI can say "unlock it" instead of
//!   "corrupt".
//! * **scanned** — a PDF that parses and has no text layer is *not* an error.
//!   It comes back as `None` so the caller can decide whether to hand the file
//!   to a vision model or tell the user it needs OCR.

use crate::{truncate_chars, DocsError, MAX_TEXT_CHARS};

/// Extract the text layer of `bytes`, or `None` when the document has none.
///
/// `name` is only used for error messages.
pub(crate) fn extract(name: &str, bytes: &[u8]) -> Result<Option<String>, DocsError> {
    // `lopdf` has to parse the trailer to know about `/Encrypt`, so a document
    // that fails at this point is broken rather than merely text-free. Since
    // pdf-extract parses with the same library, a document that fails here
    // would fail there too — we just get to explain why first.
    let doc = lopdf::Document::load_mem(bytes).map_err(|err| DocsError::PdfBroken {
        name: name.to_string(),
        reason: err.to_string(),
    })?;

    if doc.is_encrypted() {
        return Err(DocsError::PdfEncrypted {
            name: name.to_string(),
        });
    }
    drop(doc);

    let raw = pdf_extract::extract_text_from_mem(bytes).map_err(|err| DocsError::PdfBroken {
        name: name.to_string(),
        reason: err.to_string(),
    })?;

    let cleaned = normalise(&raw);
    if cleaned.is_empty() {
        // Scanned page, or a page whose text opcodes we could not decode. Both
        // are "no text layer"; an empty string would let the caller believe the
        // document is blank when it is probably a photo of a page.
        return Ok(None);
    }

    Ok(Some(truncate_chars(cleaned, MAX_TEXT_CHARS)))
}

/// Squash the artefacts of stream parsing: `\r\n` from Windows-authored PDFs,
/// stray NULs from broken embedded fonts, and the river of blank lines that
/// falls out of positioning every glyph by coordinate.
fn normalise(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank_run = 0usize;

    for line in raw.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let line = line.replace('\0', "");
        // pdf-extract emits a space per positioned glyph run; collapse the
        // resulting runs so a two-column page does not become one char per
        // token for the model.
        let line = collapse_spaces(line.trim_end());

        if line.trim().is_empty() {
            blank_run += 1;
            // Keep at most one blank line: paragraph structure is signal,
            // vertical spacing is noise.
            if blank_run > 1 {
                continue;
            }
            out.push('\n');
        } else {
            blank_run = 0;
            out.push_str(&line);
            out.push('\n');
        }
    }

    out.trim().to_string()
}

fn collapse_spaces(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_spaces = false;
    for ch in line.chars() {
        if ch == ' ' {
            if !in_spaces {
                out.push(ch);
            }
            in_spaces = true;
        } else {
            in_spaces = false;
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a one-page, valid PDF with a real xref table. Hand-assembled so the
    /// test suite carries no binary fixtures: `content` is the raw content
    /// stream, which lets a test choose between readable text and a blank page.
    fn build_pdf(content: &str) -> Vec<u8> {
        let objects = [
            String::from("<< /Type /Catalog /Pages 2 0 R >>"),
            String::from("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            String::from(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] \
                 /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
            ),
            format!(
                "<< /Length {} >>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
            String::from("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        ];

        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

        let mut offsets = Vec::with_capacity(objects.len());
        for (index, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, body).as_bytes());
        }

        let xref_at = out.len();
        let size = objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", size).as_bytes());
        for offset in &offsets {
            out.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                size, xref_at
            )
            .as_bytes(),
        );
        out
    }

    fn pdf_with_text(text: &str) -> Vec<u8> {
        build_pdf(&format!("BT /F1 24 Tf 10 50 Td ({}) Tj ET", text))
    }

    #[test]
    fn extracts_the_text_layer_of_a_real_pdf() {
        let bytes = pdf_with_text("Hello from Yura");
        let text = extract("note.pdf", &bytes).expect("parse").expect("text");
        assert!(text.contains("Hello"), "got {text:?}");
        assert!(text.contains("Yura"), "got {text:?}");
    }

    #[test]
    fn a_pdf_without_a_text_layer_is_none_not_empty() {
        let bytes = build_pdf("");
        let text = extract("scan.pdf", &bytes).expect("parse");
        assert_eq!(
            text, None,
            "a scanned page must not look like an empty string"
        );
    }

    #[test]
    fn a_broken_pdf_is_a_clear_error() {
        let bytes = b"%PDF-1.7\nthis is not a pdf body at all\n";
        let err = extract("broken.pdf", bytes).unwrap_err();
        match err {
            DocsError::PdfBroken { name, reason } => {
                assert_eq!(name, "broken.pdf");
                assert!(!reason.is_empty(), "the reason must say something");
            }
            other => panic!("expected PdfBroken, got {other:?}"),
        }
    }

    #[test]
    fn extraction_respects_the_text_cap() {
        let long = "a".repeat(MAX_TEXT_CHARS + 500);
        let cut = truncate_chars(long, MAX_TEXT_CHARS);
        assert_eq!(cut.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn truncation_never_splits_a_multibyte_char() {
        // A 3-byte character straddling the cut must be dropped whole.
        let text = "é".repeat(10);
        let cut = truncate_chars(text, 3);
        assert_eq!(cut.chars().count(), 3);
        assert!(cut.is_char_boundary(cut.len()));
    }

    #[test]
    fn normalise_keeps_paragraphs_and_drops_space_rivers() {
        let raw = "Line  one\r\n\r\n\r\nLine   two   \n";
        assert_eq!(normalise(raw), "Line one\n\nLine two");
    }
}
