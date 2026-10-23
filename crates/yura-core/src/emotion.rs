use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The emotional palette Yura can express.
///
/// This is the single vocabulary shared by three consumers: the prompt that
/// asks the model to tag its lines, the avatar that maps tags to VRM
/// expression weights, and the UI that tints the room. Adding a variant means
/// adding one match arm in `yura-avatar` — nothing else needs to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    /// The resting face.
    #[default]
    Neutral,
    /// Bright, open, smiling.
    Joy,
    /// Playful smirk, small laugh.
    Amused,
    /// Low energy, downturned.
    Sad,
    /// Brow down, sharp.
    Angry,
    /// Eyes wide, brows up.
    Surprised,
    /// Glancing away, warm cheeks.
    Shy,
    /// Eyes up and to the side, mouth closed.
    Thinking,
    /// Half-lidded, slow.
    Sleepy,
    /// Soft eyes, affectionate.
    Affection,
}

impl Emotion {
    /// Every variant, in a stable order the UI can rely on.
    pub const ALL: [Emotion; 10] = [
        Emotion::Neutral,
        Emotion::Joy,
        Emotion::Amused,
        Emotion::Sad,
        Emotion::Angry,
        Emotion::Surprised,
        Emotion::Shy,
        Emotion::Thinking,
        Emotion::Sleepy,
        Emotion::Affection,
    ];

    /// The canonical wire tag, e.g. `joy`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Emotion::Neutral => "neutral",
            Emotion::Joy => "joy",
            Emotion::Amused => "amused",
            Emotion::Sad => "sad",
            Emotion::Angry => "angry",
            Emotion::Surprised => "surprised",
            Emotion::Shy => "shy",
            Emotion::Thinking => "thinking",
            Emotion::Sleepy => "sleepy",
            Emotion::Affection => "affection",
        }
    }

    /// How strongly this emotion leans the face away from neutral, 0.0–1.0.
    /// Used as a default intensity when the model does not supply one.
    pub const fn default_intensity(self) -> f32 {
        match self {
            Emotion::Neutral => 0.0,
            Emotion::Thinking => 0.35,
            Emotion::Sleepy => 0.45,
            Emotion::Amused => 0.6,
            Emotion::Shy => 0.65,
            Emotion::Affection => 0.7,
            Emotion::Sad => 0.7,
            Emotion::Joy => 0.8,
            Emotion::Surprised => 0.8,
            Emotion::Angry => 0.75,
        }
    }
}

impl fmt::Display for Emotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Emotion {
    type Err = crate::YuraError;

    /// Accepts every spelling a model might realistically emit: `joy`,
    /// `Joy`, `happy`, `[joy]`, `<joy>`, `*joy*`, `joy:0.8`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cleaned: String = s
            .chars()
            .filter(|c| c.is_ascii_alphabetic() || *c == '_')
            .collect::<String>()
            .to_ascii_lowercase();

        let emotion = match cleaned.as_str() {
            "neutral" | "calm" | "normal" | "default" => Emotion::Neutral,
            "joy" | "happy" | "happiness" | "cheerful" | "excited" => Emotion::Joy,
            "amused" | "playful" | "smug" | "laugh" | "laughing" | "teasing" => Emotion::Amused,
            "sad" | "sadness" | "down" | "hurt" | "lonely" => Emotion::Sad,
            "angry" | "anger" | "annoyed" | "mad" | "pout" | "pouting" => Emotion::Angry,
            "surprised" | "surprise" | "shock" | "shocked" | "startled" => Emotion::Surprised,
            "shy" | "embarrassed" | "blush" | "flustered" | "bashful" => Emotion::Shy,
            "thinking" | "think" | "curious" | "confused" | "ponder" => Emotion::Thinking,
            "sleepy" | "tired" | "drowsy" | "yawn" => Emotion::Sleepy,
            "affection" | "love" | "warm" | "tender" | "fond" | "cute" => Emotion::Affection,
            _ => return Err(crate::YuraError::UnknownEmotion(s.to_owned())),
        };
        Ok(emotion)
    }
}

/// Parse a tag, falling back to [`Emotion::Neutral`] instead of failing.
/// The model is a language model; a stray tag must never break a reply.
pub fn parse_or_neutral(s: &str) -> Emotion {
    Emotion::from_str(s).unwrap_or(Emotion::Neutral)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_spellings() {
        for raw in ["joy", "HAPPY", "[joy]", "<Amused>", "*shy*", "joy:0.8"] {
            let e = Emotion::from_str(raw).expect(raw);
            assert!(matches!(e, Emotion::Joy | Emotion::Amused | Emotion::Shy));
        }
    }

    #[test]
    fn unknown_tags_degrade_gracefully() {
        assert_eq!(parse_or_neutral("benevolent"), Emotion::Neutral);
        assert_eq!("sleepy".parse::<Emotion>().unwrap().as_str(), "sleepy");
    }

    #[test]
    fn display_round_trips() {
        for e in Emotion::ALL {
            assert_eq!(e.as_str().parse::<Emotion>().unwrap(), e);
        }
    }
}
