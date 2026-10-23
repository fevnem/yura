//! # yura-core
//!
//! The domain layer. Everything here is pure data plus the rules that govern
//! it — no I/O, no platform code, no UI. Both the Rust side and (through
//! `serde`) the webview speak these types, so the contracts live in exactly
//! one place.
//!
//! Layers above this crate:
//! * [`yura_llm`] — turns [`Message`]s into streamed [`Reply`]s
//! * [`yura_avatar`] — turns [`Emotion`]s into face weights
//! * [`yura_store`] — persists [`Settings`] and conversation history

pub mod emotion;
pub mod error;
pub mod message;
pub mod persona;
pub mod reply;
pub mod settings;

pub use emotion::Emotion;
pub use error::{Result, YuraError};
pub use message::{Message, Role};
pub use persona::Persona;
pub use reply::{parse, Reply, ReplyParser, Utterance};
pub use settings::{
    preset, AvatarSettings, LlmSettings, ProviderKind, ProviderPreset, Settings, UiSettings,
    VoiceSettings, PRESETS, SETTINGS_VERSION,
};
