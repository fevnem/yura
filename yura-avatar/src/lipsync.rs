use serde::{Deserialize, Serialize};

/// A mouth shape and the spectral signature that implies it.
///
/// Real viseme extraction needs phoneme timings, which no browser TTS gives us.
/// Instead the renderer analyses the speaking spectrum and projects it onto
/// these archetypes, so the mouth tracks loudness *and* brightness — an "ee"
/// looks different from an "oh" even at the same volume.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Viseme {
    /// VRM preset this shape drives, e.g. `aa`.
    pub preset: &'static str,
    /// How much low-band energy (≈ open vowel) this shape wants.
    pub low: f32,
    /// Mid-band energy (≈ neutral vowel).
    pub mid: f32,
    /// High-band energy (≈ wide vowel / sibilance).
    pub high: f32,
}

/// Ordered so that index 0 is the rest shape.
///
/// Each row is a *unit emphasis* across the three bands (it sums to 1.0), so
/// no viseme wins purely by being louder than its neighbours — only by which
/// band it emphasises.
pub const VISEMES: &[Viseme] = &[
    Viseme {
        preset: "aa",
        low: 0.87,
        mid: 0.13,
        high: 0.0,
    },
    Viseme {
        preset: "oh",
        low: 0.54,
        mid: 0.46,
        high: 0.0,
    },
    Viseme {
        preset: "ou",
        low: 0.38,
        mid: 0.58,
        high: 0.04,
    },
    Viseme {
        preset: "ee",
        low: 0.07,
        mid: 0.22,
        high: 0.71,
    },
    Viseme {
        preset: "ih",
        low: 0.14,
        mid: 0.61,
        high: 0.25,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_the_vrm_mouth_shapes() {
        for v in VISEMES {
            assert!(
                matches!(v.preset, "aa" | "ih" | "ou" | "ee" | "oh"),
                "{} is not a VRM mouth preset",
                v.preset
            );
        }
    }

    #[test]
    fn spectral_weights_are_normalised() {
        for v in VISEMES {
            let total = v.low + v.mid + v.high;
            assert!(
                (0.98..=1.02).contains(&total),
                "{} bands sum to {total}, expected 1.0",
                v.preset
            );
        }
    }

    #[test]
    fn shapes_are_actually_distinguishable() {
        // If two archetypes were near-identical the mouth would not change
        // shape between vowels, which is the whole point of the table.
        for (i, a) in VISEMES.iter().enumerate() {
            for b in &VISEMES[i + 1..] {
                let distance = (a.low - b.low).abs() + (a.mid - b.mid).abs() + (a.high - b.high).abs();
                assert!(
                    distance > 0.25,
                    "{} and {} are too similar ({distance})",
                    a.preset,
                    b.preset
                );
            }
        }
    }
}
