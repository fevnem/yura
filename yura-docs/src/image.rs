//! Image attachments.
//!
//! Images are never decoded: a screenshot is worth more to the model as the
//! original bytes than as anything we could re-encode, and decoding in-process
//! would mean an image codec in the binary. The renderer shows the picture and a
//! multimodal model receives the same data URL, so one string serves both.

use base64::Engine as _;

/// Build a `data:` URL the webview can put straight into an `<img src>` and an
/// OpenAI-style `image_url` part can accept verbatim.
pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{mime};base64,{encoded}")
}

/// Pull the MIME type back out of a data URL, for prompt headers and for the UI
/// chip label. Returns `None` for anything that is not a well-formed data URL.
pub(crate) fn mime_from_data_url(data_url: &str) -> Option<&str> {
    let rest = data_url.strip_prefix("data:")?;
    let (mime, _) = rest.split_once(";base64,")?;
    (!mime.is_empty()).then_some(mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_standard_data_url() {
        let url = data_url("image/png", &[0x89, b'P', b'N', b'G']);
        assert_eq!(url, "data:image/png;base64,iVBORw==");
    }

    #[test]
    fn reads_the_mime_type_back() {
        let url = data_url("image/jpeg", &[0xff, 0xd8, 0xff]);
        assert_eq!(mime_from_data_url(&url), Some("image/jpeg"));
        assert_eq!(mime_from_data_url("https://x/y.png"), None);
        assert_eq!(mime_from_data_url("data:,"), None);
    }
}
