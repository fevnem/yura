//! # yura-llm
//!
//! Turns a conversation into a stream of tokens.
//!
//! The public surface is deliberately tiny:
//! * [`LlmProvider`] — the trait every backend implements
//! * [`build`] — constructs the right backend from [`yura_core::LlmSettings`]
//! * [`ChatRequest`] — what the app hands over
//!
//! Adding a vendor means adding one module and one match arm in
//! [`registry`]. Nothing above this crate changes.

pub mod anthropic;
pub mod error;
pub mod http;
pub mod openai;
pub mod provider;
pub mod registry;
pub mod sse;

pub use provider::{ChatRequest, LlmProvider};
pub use registry::build;
