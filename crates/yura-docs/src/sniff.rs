//! Magic-byte sniffing.
//!
//! Extensions lie: users rename `invoice.pdf` to `.png`, drag a screenshot out
//! of a clipboard that calls it `.tmp`, and mail clients happily deliver
//! `.dat`. Every decision in this crate is therefore made from the bytes, and
//! the file name is used only as a label.

use crate::AttachmentKind;

/// What the first few hundred bytes say the file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sniffed {
    pub kind: AttachmentKind,
    /// MIME type for images, `None` for everything else. The renderer needs it
    /// to label the chip and the model needs it in the data URL.
    pub mime: Option<&'static str>,
}

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// The PDF spec allows leading junk before `%PDF-`, as long as the header
/// appears within the first 1024 bytes. Some scanners use that slack, so we
/// search instead of only checking the very first bytes.
const PDF_HEADER_SCAN: usize = 1024;

pub(crate) fn sniff(bytes: &[u8]) -> Sniffed {
    if is_pdf(bytes) {
        return Sniffed {
            kind: AttachmentKind::Pdf,
            mime: Some("application/pdf"),
        };
    }

    if let Some(mime) = image_mime(bytes) {
        return Sniffed {
            kind: AttachmentKind::Image,
            mime: Some(mime),
        };
    }

    Sniffed {
        kind: AttachmentKind::Other,
        mime: None,
    }
}

fn is_pdf(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(PDF_HEADER_SCAN)];
    window.windows(5).any(|chunk| chunk == b"%PDF-")
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&PNG_MAGIC) {
        return Some("image/png");
    }
    // Every JPEG starts with the SOI marker followed by another marker.
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    // RIFF container with a WEBP form type.
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
    ];
    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
    const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00";
    const WEBP: &[u8] = b"RIFF\x24\x00\x00\x00WEBPVP8 ";

    #[test]
    fn sniffs_images_from_magic_bytes() {
        assert_eq!(sniff(PNG).kind, AttachmentKind::Image);
        assert_eq!(sniff(PNG).mime, Some("image/png"));
        assert_eq!(sniff(JPEG).mime, Some("image/jpeg"));
        assert_eq!(sniff(GIF).mime, Some("image/gif"));
        assert_eq!(sniff(WEBP).mime, Some("image/webp"));
    }

    #[test]
    fn sniffs_pdf_even_with_a_prepended_garbage_byte() {
        let mut bytes = vec![b'\n'];
        bytes.extend_from_slice(b"%PDF-1.7\n");
        assert_eq!(sniff(&bytes).kind, AttachmentKind::Pdf);
    }

    #[test]
    fn plain_text_and_unknown_binaries_are_not_special() {
        assert_eq!(sniff(b"# notes\n\nhello").kind, AttachmentKind::Other);
        assert_eq!(sniff(&[0x00, 0x01, 0x02, 0x03]).kind, AttachmentKind::Other);
        assert_eq!(sniff(b"").kind, AttachmentKind::Other);
    }

    #[test]
    fn riff_that_is_not_webp_stays_opaque() {
        let wav = b"RIFF\x24\x00\x00\x00WAVEfmt ";
        assert_eq!(sniff(wav).kind, AttachmentKind::Other);
    }
}
