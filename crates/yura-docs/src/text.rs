//! Plain-text ingestion.
//!
//! Text files are the one case where guessing costs nothing: a UTF-8 decode
//! that succeeds is almost certainly prose, and a decode that fails is either
//! latin-1 snuck in by an old editor or a binary we should not pretend to
//! understand. The heuristic here is deliberately conservative — it is worse
//! to hand a model 200 kB of mojibake than to show a chip that says "binary".

/// Decode `bytes` as text, or `None` when it looks binary.
pub(crate) fn decode(bytes: &[u8]) -> Option<String> {
    // Embedded NULs mean a container format (executables, archives, images that
    // slipped past the sniffer). Valid UTF-8 can still contain them, so this
    // check comes first.
    if bytes.contains(&0) {
        return None;
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return Some(text.to_string());
    }

    // Invalid UTF-8: accept it only if nearly every byte is printable ASCII,
    // which is what latin-1 or cp1252 text looks like. 90% leaves room for
    // accented words while rejecting anything with structure.
    let printable = bytes
        .iter()
        .filter(|byte| matches!(byte, b'\n' | b'\r' | b'\t' | 0x20..=0x7e))
        .count();
    let ratio = printable as f32 / bytes.len() as f32;
    if ratio >= 0.90 {
        return Some(String::from_utf8_lossy(bytes).into_owned());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_text_passes_through_unchanged() {
        let raw = "# Title\n\n- 日本語もそのまま\n";
        assert_eq!(decode(raw.as_bytes()).as_deref(), Some(raw));
    }

    #[test]
    fn empty_file_is_empty_text_not_binary() {
        assert_eq!(decode(b"").as_deref(), Some(""));
    }

    #[test]
    fn latin1_text_is_recovered_lossily() {
        // 0xe9 / 0xea are 'é' / 'ê' in latin-1 and invalid on their own in
        // UTF-8, which is exactly the shape of an old exported .txt file.
        let bytes = b"Le caf\xe9 est pr\xeat sur la table.\n";
        let text = decode(bytes).expect("should be treated as text");
        assert!(text.contains("Le caf"), "got {text:?}");
        assert!(text.contains('\u{fffd}'), "lossy replacement expected");
    }

    #[test]
    fn a_single_binary_stray_byte_still_reads_as_text() {
        // One cp1252 quote in an otherwise ASCII file must not demote the whole
        // document to an opaque chip.
        let bytes = b"He said \x93hello\x94 and left.\n";
        assert!(decode(bytes).is_some());
    }

    #[test]
    fn nul_bytes_make_a_file_binary() {
        let bytes = b"PK\x03\x04\x00\x00\x00\x00";
        assert_eq!(decode(bytes), None);
    }

    #[test]
    fn high_entropy_bytes_are_not_text() {
        // No NULs, invalid UTF-8, mostly unprintable: an encrypted blob.
        let bytes: Vec<u8> = (0u8..=255).filter(|b| *b >= 0x80).collect();
        assert_eq!(decode(&bytes), None);
    }
}
