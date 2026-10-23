use serde::{Deserialize, Serialize};
use yura_core::Emotion;

/// One emotion expressed as VRM preset weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionDef {
    /// Wire tag, e.g. `shy`.
    pub emotion: Emotion,
    /// Preset name → weight, 0.0–1.0. Names follow the VRM 0.x/1.0 presets
    /// plus the extra mouth shapes VRoid exports; the renderer ignores names
    /// the model does not have, so this stays portable across characters.
    pub weights: Vec<(String, f32)>,
}

/// Timings and amplitudes for the things that keep the character alive when
/// nobody is talking. Values are sensible defaults; the renderer may override
/// them from settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleParams {
    /// Seconds per full breath cycle.
    pub breath_period: f32,
    /// Breathing rotation amplitude, radians.
    pub breath_amplitude: f32,
    /// Seconds between blinks, typical.
    pub blink_interval: f32,
    /// Random extra delay, so blinks never look metronomic.
    pub blink_jitter: f32,
    /// Diagonal of the gaze wander box, metres at 1 m.
    pub gaze_wander: f32,
    /// How long a gaze target is held before drifting, seconds.
    pub gaze_hold: f32,
    /// Head follows the cursor at this fraction of the gaze angle.
    pub head_follow: f32,
}

impl Default for IdleParams {
    fn default() -> Self {
        Self {
            breath_period: 4.2,
            breath_amplitude: 0.012,
            blink_interval: 4.6,
            blink_jitter: 3.2,
            gaze_wander: 0.09,
            gaze_hold: 1.8,
            head_follow: 0.28,
        }
    }
}

/// Everything the renderer needs to animate a face, minus the mesh itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarSpec {
    pub version: u32,
    pub expressions: Vec<ExpressionDef>,
    pub idle: IdleParams,
}

/// The full presets table. One row per [`Emotion`]; weights are hand-tuned to
/// read clearly at a glance, which matters more than anatomical accuracy on a
/// stylised character.
pub const EXPRESSIONS: &[(Emotion, &[(&str, f32)])] = &[
    (Emotion::Neutral, &[]),
    (
        Emotion::Joy,
        &[("happy", 0.85), ("relaxed", 0.25), ("blink", 0.05)],
    ),
    (
        Emotion::Amused,
        &[("happy", 0.55), ("relaxed", 0.35), ("aa", 0.12)],
    ),
    (Emotion::Sad, &[("sad", 0.8), ("blink", 0.15)]),
    (Emotion::Angry, &[("angry", 0.75)]),
    (
        Emotion::Surprised,
        &[("surprised", 0.9), ("oh", 0.15)],
    ),
    (
        Emotion::Shy,
        &[("happy", 0.3), ("relaxed", 0.3), ("blink", 0.2)],
    ),
    (Emotion::Thinking, &[("relaxed", 0.2), ("blinkLeft", 0.1)]),
    (
        Emotion::Sleepy,
        &[("relaxed", 0.55), ("blink", 0.45), ("blinkLeft", 0.15)],
    ),
    (
        Emotion::Affection,
        &[("happy", 0.45), ("relaxed", 0.4), ("blink", 0.25)],
    ),
];

/// Build the spec handed to the renderer at boot.
pub fn spec() -> AvatarSpec {
    let expressions = EXPRESSIONS
        .iter()
        .map(|(emotion, weights)| ExpressionDef {
            emotion: *emotion,
            weights: weights
                .iter()
                .map(|(k, v)| ((*k).to_owned(), *v))
                .collect(),
        })
        .collect();

    AvatarSpec {
        version: 1,
        expressions,
        idle: IdleParams::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_emotion_has_exactly_one_row() {
        for e in Emotion::ALL {
            let n = EXPRESSIONS.iter().filter(|(x, _)| *x == e).count();
            assert_eq!(n, 1, "emotion {e} must appear once");
        }
        assert_eq!(EXPRESSIONS.len(), Emotion::ALL.len());
    }

    #[test]
    fn weights_are_unit_interval_and_positive() {
        for (emotion, weights) in EXPRESSIONS {
            for (name, w) in *weights {
                assert!(
                    (0.0..=1.0).contains(w),
                    "{emotion}/{name} weight {w} out of range"
                );
                assert!(*w > 0.0, "{emotion}/{name} is a no-op weight");
            }
        }
    }

    #[test]
    fn mouth_presets_are_never_driven_by_mood() {
        // Mouth shapes belong to lip-sync; an emotion that pins one fights the
        // audio every frame. `aa`/`oh` appear only as tiny accents above.
        for (emotion, weights) in EXPRESSIONS {
            let mouth: f32 = weights
                .iter()
                .filter(|(n, _)| matches!(*n, "aa" | "ih" | "ou" | "ee" | "oh"))
                .map(|(_, w)| *w)
                .sum();
            assert!(mouth <= 0.2, "{emotion} pins the mouth too hard: {mouth}");
        }
    }

    #[test]
    fn idle_defaults_are_plausible() {
        let p = IdleParams::default();
        assert!(p.breath_period > 2.0 && p.breath_period < 8.0);
        assert!(p.blink_interval > 1.0);
        assert!((0.0..=1.0).contains(&p.head_follow));
    }
}
