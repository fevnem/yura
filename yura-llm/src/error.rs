use yura_core::YuraError;

/// Flatten a transport error into the domain error type.
///
/// `yura-core` deliberately does not depend on `reqwest`; this is the single
/// place the conversion happens for every provider.
pub(crate) fn net(error: reqwest::Error) -> YuraError {
    YuraError::Network(error.to_string())
}
