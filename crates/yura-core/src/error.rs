use thiserror::Error;

pub type Result<T, E = YuraError> = std::result::Result<T, E>;

/// Everything that can go wrong inside the domain, and nowhere else.
/// Transport errors are converted at the crate boundary so that the app layer
/// only has to render one error type.
#[derive(Debug, Error)]
pub enum YuraError {
    #[error("unknown emotion tag: {0}")]
    UnknownEmotion(String),

    #[error("no provider configured")]
    NoProvider,

    #[error("missing API key for provider '{0}'")]
    MissingApiKey(String),

    #[error("provider '{provider}' returned {status}: {body}")]
    Provider {
        provider: String,
        status: u16,
        body: String,
    },

    /// Transport failure, flattened to a string so the domain layer stays free
    /// of HTTP dependencies. `yura-llm` converts at its boundary.
    #[error("network error: {0}")]
    Network(String),

    #[error("could not parse provider response: {0}")]
    Decode(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
