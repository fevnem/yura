//! # yura-avatar
//!
//! The face, described as data.
//!
//! VRM models expose a fixed set of expression presets (`happy`, `angry`,
//! `aa`, `blink`, …). Rather than scattering those magic strings through the
//! TypeScript renderer, the mapping lives here and is shipped to the webview
//! as one JSON [`AvatarSpec`] at boot. That gives us a single source of truth:
//! retune an emotion once and both the renderer and any future exporter agree.
//!
//! The crate has no I/O and no platform code — it is a pure lookup table plus
//! the small amount of reasoning the renderer would otherwise duplicate.

pub mod expression;
pub mod lipsync;

pub use expression::{spec, AvatarSpec, ExpressionDef, IdleParams};
pub use lipsync::{Viseme, VISEMES};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_serializable_and_complete() {
        let s = spec();
        let json = serde_json::to_string(&s).expect("spec serializes");
        assert!(json.contains("happy"));
        assert_eq!(s.expressions.len(), yura_core::Emotion::ALL.len());
    }
}
