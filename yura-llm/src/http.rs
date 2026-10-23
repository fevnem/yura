use std::time::Duration;

use reqwest::Client;
use std::sync::OnceLock;

static CLIENT: OnceLock<Client> = OnceLock::new();

/// One shared HTTP client for the whole process.
///
/// Reusing the connection pool matters here: a companion app chats in short
/// bursts, and a TLS handshake per turn is the difference between a reply that
/// feels instant and one that feels laggy.
pub fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(15))
            // Streaming bodies can legitimately stay open for a long time, so
            // there is no total timeout — only a read timeout between bytes.
            .read_timeout(Duration::from_secs(120))
            .user_agent(concat!("Yura/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build HTTP client")
    })
}
