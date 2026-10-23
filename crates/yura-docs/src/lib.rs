//! # yura-docs
//!
//! Turns a file that was dragged onto the window into something a model can
//! read.
//!
//! The whole crate exists to answer one question per file: *what is the most
//! useful representation of this for a chat turn?* Three answers are possible,
//! and the caller picks one with [`ingest_path`] / [`ingest_bytes`]:
//!
//! * **text** — the extracted text layer of a PDF, or the decoded contents of a
//!   text file, capped at [`MAX_TEXT_CHARS`] so a 900-page book cannot blow the
//!   context window.
//! * **image** — a `data:` URL, so the renderer can paint a thumbnail and a
//!   multimodal model can receive the same bytes.
//! * **other** — a chip with a name and a size. Not an error: a `.zip` or a
//!   `.exe` is a legitimate thing to drop, we just cannot read it.
//!
//! Everything here is pure Rust on purpose. Yura is cross-compiled from Linux to
//! `x86_64-pc-windows-msvc` with `cargo-xwin`, so a dependency that wants CMake,
//! a C compiler or a bundled `.so` cannot be linked at all. That is also why OCR
//! is not offered: see [`ocr_text`].
//!
//! ## Honesty rules
//!
//! Two of them, both load-bearing:
//!
//! * A PDF with no text layer yields `text: None`, never `Some("")`. "Scanned"
//!   and "blank" are different facts, and the caller (and the user) has to be
//!   able to tell them apart.
//! * [`to_prompt_block`] says what it is handing over, including what it left
//!   out. A model that is told `[…truncated]` will not claim to have read the
//!   last chapter.

mod image;
mod pdf;
mod sniff;
mod text;

use std::path::Path;

/// Cap on stored text, in characters. Roughly 50k tokens — generous for an
/// article, small enough that a book does not evict the conversation.
pub const MAX_TEXT_CHARS: usize = 200_000;

/// Largest file we will read into memory at all. Beyond this we would rather
/// refuse than let a mis-drop of a 4 GB ISO stall the app.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// What the file turned out to be, decided from its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Pdf,
    Image,
    Text,
    Other,
}

impl AttachmentKind {
    /// Short stable label for prompts, logs and UI chips.
    pub fn as_str(self) -> &'static str {
        match self {
            AttachmentKind::Pdf => "pdf",
            AttachmentKind::Image => "image",
            AttachmentKind::Text => "text",
            AttachmentKind::Other => "other",
        }
    }
}

impl std::fmt::Display for AttachmentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One dropped file, resolved to whatever we could get out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// File name as the user knows it. Used as a label, never to decide type.
    pub name: String,
    /// Decided from magic bytes, so a renamed `.png` is still an image.
    pub kind: AttachmentKind,
    /// Size of the original file, so the UI can show a chip even for [`AttachmentKind::Other`].
    pub bytes: usize,
    /// Extracted plain text, for PDFs and text files. `None` for images, for
    /// binaries, and for PDFs that have no text layer (scans).
    pub text: Option<String>,
    /// `data:image/png;base64,...` for images, so the renderer can show it and a
    /// multimodal model can receive it.
    pub data_url: Option<String>,
    /// True when the text was recovered by OCR rather than a text layer.
    ///
    /// Always `false` today: no pure-Rust OCR engine is bundled, so scanned
    /// pages come back with `text: None` instead. The field exists so callers
    /// can already distinguish recovered text from extracted text, and so the
    /// day a backend lands nothing outside this crate has to change.
    pub ocr: bool,
}

impl Attachment {
    /// True when there is text a model can read.
    pub fn has_text(&self) -> bool {
        self.text
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty())
    }
}

/// Errors raised while turning a file into an [`Attachment`].
#[derive(Debug, thiserror::Error)]
pub enum DocsError {
    #[error("could not read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{name} is {bytes} bytes, which is over the {limit} byte attachment limit")]
    TooLarge {
        name: String,
        bytes: usize,
        limit: usize,
    },

    #[error("{name} is password-protected; remove the password and drop it again")]
    PdfEncrypted { name: String },

    #[error("{name} could not be parsed as a PDF: {reason}")]
    PdfBroken { name: String, reason: String },

    /// OCR is deliberately not implemented. See [`ocr_text`].
    #[error(
        "OCR is not supported: Yura does not ship an OCR engine, so scanned pages and image-only \
         PDFs have no text. Paste the text, or ask a vision-capable model to read the image."
    )]
    OcrUnsupported,
}

pub type Result<T, E = DocsError> = std::result::Result<T, E>;

/// Ingest bytes already in memory. `name` is a label only.
pub fn ingest_bytes(name: &str, bytes: &[u8]) -> Result<Attachment> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(DocsError::TooLarge {
            name: name.to_string(),
            bytes: bytes.len(),
            limit: MAX_INPUT_BYTES,
        });
    }

    let sniffed = sniff::sniff(bytes);
    tracing::debug!(
        attachment = name,
        kind = sniffed.kind.as_str(),
        bytes = bytes.len(),
        "ingesting attachment"
    );

    let mut attachment = Attachment {
        name: name.to_string(),
        kind: sniffed.kind,
        bytes: bytes.len(),
        text: None,
        data_url: None,
        ocr: false,
    };

    match sniffed.kind {
        AttachmentKind::Pdf => {
            attachment.text = pdf::extract(name, bytes)?;
        }
        AttachmentKind::Image => {
            // `sniff` always names a MIME type for images; the fallback keeps
            // the signature total rather than unwrapping.
            let mime = sniffed.mime.unwrap_or("application/octet-stream");
            attachment.data_url = Some(image::data_url(mime, bytes));
        }
        // Not a PDF, not an image: text if it decodes as such, opaque if not.
        // A binary is a valid attachment, just not a readable one.
        _ => match text::decode(bytes) {
            Some(decoded) => {
                attachment.kind = AttachmentKind::Text;
                attachment.text = Some(truncate_chars(decoded, MAX_TEXT_CHARS));
            }
            None => {
                attachment.kind = AttachmentKind::Other;
            }
        },
    }

    Ok(attachment)
}

/// Read and ingest a file from disk.
///
/// Reads in one go: attachments are capped at [`MAX_INPUT_BYTES`], and streaming
/// a 2 MB PDF through a buffer would only add code.
pub fn ingest_path(path: &Path) -> Result<Attachment> {
    // Check the size before reading so a huge file does not have to be
    // allocated first.
    let metadata = std::fs::metadata(path).map_err(|source| DocsError::Io {
        path: path.display().to_string(),
        source,
    })?;
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err(DocsError::TooLarge {
            name: display_name(path),
            bytes: metadata.len() as usize,
            limit: MAX_INPUT_BYTES,
        });
    }

    let bytes = std::fs::read(path).map_err(|source| DocsError::Io {
        path: path.display().to_string(),
        source,
    })?;

    ingest_bytes(&display_name(path), &bytes)
}

/// Recover text from a page that has no text layer.
///
/// **This always fails with [`DocsError::OcrUnsupported`].** OCR is not wired up
/// because the pure-Rust options are either immature or large enough to double
/// the installer, and anything else (`tesseract`, `pdfium`) breaks the
/// `x86_64-pc-windows-msvc` cross-build from Linux. Rather than ship a stub that
/// returns empty text — which would read to a model as "this document is blank"
/// — the function refuses, and [`to_prompt_block`] tells the model the page is a
/// scan.
///
/// The escape hatch is a vision-capable model: pass the image itself.
pub fn ocr_text(_bytes: &[u8]) -> Result<String> {
    Err(DocsError::OcrUnsupported)
}

/// Render an attachment as a compact block for a prompt.
///
/// The block always opens with a one-line header naming the file and its kind,
/// because a model that does not know what it is reading will invent context.
/// The body is the text, cut to fit `max_chars` with an explicit
/// `[…truncated]` marker, or a bracketed statement of *why* there is no text.
///
/// The header is never truncated, and a cut body always keeps its marker, so a
/// `max_chars` smaller than header + marker overflows by a few characters.
/// Dropping either would leave the model misreading the fragment.
pub fn to_prompt_block(attachment: &Attachment, max_chars: usize) -> String {
    let mut block = String::with_capacity(max_chars.saturating_add(128).min(MAX_TEXT_CHARS));

    block.push_str("[attachment] ");
    block.push_str(&attachment.name);
    block.push_str(" (");
    match attachment.kind {
        // `image/png` already says "image", so the chip stays one item long.
        AttachmentKind::Image => {
            let mime = attachment
                .data_url
                .as_deref()
                .and_then(image::mime_from_data_url)
                .unwrap_or("image");
            block.push_str(mime);
        }
        other => block.push_str(other.as_str()),
    }
    block.push_str(", ");
    block.push_str(&human_size(attachment.bytes));
    block.push(')');

    match attachment.text.as_deref() {
        Some(text) if !text.trim().is_empty() => {
            block.push('\n');
            // Count the newline we just wrote: `max_chars` is a ceiling on the
            // whole block, not just on the body.
            let budget = max_chars.saturating_sub(block.chars().count());
            push_capped(&mut block, text, budget);
        }
        // A text file that is genuinely empty, or whitespace only.
        Some(_) => block.push_str("\n[empty file]"),
        None => {
            block.push('\n');
            block.push_str(match attachment.kind {
                // Deliberately not "the document is blank": a scanned page is
                // full of information we simply cannot read.
                AttachmentKind::Pdf => "[no extractable text — scanned image]",
                AttachmentKind::Image => "[image data attached separately — no text layer]",
                AttachmentKind::Text => "[empty file]",
                AttachmentKind::Other => "[no extractable text — binary or unsupported format]",
            });
        }
    }

    block
}

const TRUNCATED_MARKER: &str = "[…truncated]";

/// Append `text` to `out`, keeping the total addition within `budget` chars.
fn push_capped(out: &mut String, text: &str, budget: usize) {
    let len = text.chars().count();
    if len <= budget {
        out.push_str(text.trim_end());
        return;
    }

    // The marker needs its own line, so reserve the newline before it too.
    let marker_len = TRUNCATED_MARKER.chars().count() + 1;
    let room = budget.saturating_sub(marker_len);
    let cut = text
        .char_indices()
        .nth(room)
        .map(|(index, _)| index)
        .unwrap_or(text.len());

    out.push_str(text[..cut].trim_end());
    out.push('\n');
    out.push_str(TRUNCATED_MARKER);
}

/// Cut a `String` at a character boundary, never mid-UTF-8.
pub(crate) fn truncate_chars(text: String, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text;
    }
    let cut = text
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    text[..cut].to_string()
}

/// Human-readable byte count, for the header line and UI chips.
pub(crate) fn human_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let bytes_f = bytes as f64;
    if bytes_f < KB {
        format!("{bytes} B")
    } else if bytes_f < MB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{:.1} MB", bytes_f / MB)
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R',
    ];

    #[test]
    fn an_image_becomes_a_data_url_with_no_text() {
        let attachment = ingest_bytes("shot.png", PNG).unwrap();

        assert_eq!(attachment.kind, AttachmentKind::Image);
        assert_eq!(attachment.bytes, PNG.len());
        assert_eq!(attachment.text, None);
        assert!(!attachment.ocr);
        let url = attachment.data_url.expect("data url");
        assert!(url.starts_with("data:image/png;base64,"), "got {url}");
    }

    #[test]
    fn the_extension_never_decides_the_kind() {
        // A PNG wearing a PDF name is still an image; that is the whole point of
        // sniffing magic bytes.
        let attachment = ingest_bytes("invoice.pdf", PNG).unwrap();
        assert_eq!(attachment.kind, AttachmentKind::Image);
        assert!(attachment.text.is_none());
    }

    #[test]
    fn text_passes_through_and_reaches_the_prompt_block() {
        let source = "# Notes\n\nbuy milk\n";
        let attachment = ingest_bytes("notes.md", source.as_bytes()).unwrap();

        assert_eq!(attachment.kind, AttachmentKind::Text);
        assert_eq!(attachment.text.as_deref(), Some(source));

        let block = to_prompt_block(&attachment, 10_000);
        assert!(
            block.starts_with("[attachment] notes.md (text, "),
            "got {block}"
        );
        assert!(block.contains("buy milk"));
        assert!(!block.contains(TRUNCATED_MARKER));
    }

    #[test]
    fn an_unknown_binary_is_an_other_chip_not_an_error() {
        let bytes = [0x4d, 0x5a, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00]; // MZ header
        let attachment = ingest_bytes("setup.exe", &bytes).unwrap();

        assert_eq!(attachment.kind, AttachmentKind::Other);
        assert_eq!(attachment.text, None);
        assert_eq!(attachment.data_url, None);
        assert_eq!(attachment.bytes, bytes.len());

        let block = to_prompt_block(&attachment, 1_000);
        assert!(block.contains("setup.exe (other, 8 B)"), "got {block}");
        assert!(block.contains("[no extractable text"));
    }

    #[test]
    fn long_text_is_cut_with_an_explicit_marker() {
        let source = "word ".repeat(100);
        let attachment = ingest_bytes("long.txt", source.as_bytes()).unwrap();

        let block = to_prompt_block(&attachment, 100);
        assert!(block.ends_with(TRUNCATED_MARKER), "got {block}");
        assert!(
            block.chars().count() <= 100,
            "over budget: {}",
            block.chars().count()
        );
        // The header survives truncation, so the model still knows the source.
        assert!(block.starts_with("[attachment] long.txt (text,"));
    }

    #[test]
    fn truncation_is_utf8_safe_and_respects_the_budget() {
        let source = "é".repeat(500);
        let attachment = ingest_bytes("accents.txt", source.as_bytes()).unwrap();

        for budget in [120usize, 160, 400] {
            let block = to_prompt_block(&attachment, budget);
            assert!(block.chars().count() <= budget, "budget {budget} blown");
            assert!(block.contains(TRUNCATED_MARKER));
        }
    }

    #[test]
    fn a_short_budget_still_names_the_file_and_marks_the_cut() {
        let attachment = ingest_bytes("a.txt", b"hello").unwrap();
        let block = to_prompt_block(&attachment, 0);
        // Documented overflow: metadata is never sacrificed to the ceiling.
        assert!(
            block.starts_with("[attachment] a.txt (text, 5 B)"),
            "got {block}"
        );
        assert!(block.ends_with(TRUNCATED_MARKER), "got {block}");
    }

    #[test]
    fn a_scanned_pdf_says_so_instead_of_pretending_to_be_empty() {
        let attachment = Attachment {
            name: "scan.pdf".into(),
            kind: AttachmentKind::Pdf,
            bytes: 1_048_576,
            text: None,
            data_url: None,
            ocr: false,
        };

        let block = to_prompt_block(&attachment, 4_000);
        assert!(block.contains("scan.pdf (pdf, 1.0 MB)"), "got {block}");
        assert!(
            block.contains("[no extractable text — scanned image]"),
            "got {block}"
        );
        assert!(!attachment.has_text());
    }

    #[test]
    fn empty_text_file_is_labelled_empty() {
        let attachment = ingest_bytes("blank.txt", b"   \n").unwrap();
        let block = to_prompt_block(&attachment, 1_000);
        assert!(block.contains("[empty file]"), "got {block}");
    }

    #[test]
    fn ingest_path_reads_from_disk_and_labels_with_the_file_name() {
        let dir = std::env::temp_dir().join(format!("yura-docs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("todo.csv");
        std::fs::write(&path, "id,task\n1,ship it\n").unwrap();

        let attachment = ingest_path(&path).unwrap();
        assert_eq!(attachment.name, "todo.csv");
        assert_eq!(attachment.kind, AttachmentKind::Text);
        assert_eq!(attachment.text.as_deref(), Some("id,task\n1,ship it\n"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_is_an_io_error_naming_the_path() {
        let path = std::path::Path::new("/nonexistent/yura/nope.pdf");
        match ingest_path(path) {
            Err(DocsError::Io { path: reported, .. }) => assert!(reported.contains("nope.pdf")),
            other => panic!("expected Io, got {other:?}"),
        }
    }

    #[test]
    fn ocr_refuses_instead_of_returning_nothing() {
        let err = ocr_text(PNG).unwrap_err();
        assert!(matches!(err, DocsError::OcrUnsupported));
        assert!(err.to_string().contains("OCR is not supported"));
    }

    #[test]
    fn sizes_render_for_chips() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(999), "999 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
