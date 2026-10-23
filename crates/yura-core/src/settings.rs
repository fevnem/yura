use serde::{Deserialize, Serialize};

use crate::Persona;

/// Bumped whenever the on-disk shape changes in a way that needs migration.
pub const SETTINGS_VERSION: u32 = 1;

/// Which wire protocol a provider speaks.
///
/// Almost every vendor now exposes an OpenAI-compatible `/chat/completions`
/// endpoint — OpenAI, DeepSeek, OpenRouter, Groq, Mistral, xAI, Together,
/// Ollama, LM Studio, llama.cpp's server. One implementation covers them all;
/// Anthropic's Messages API is the lone special case worth its own code path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAiCompatible,
    Anthropic,
}

impl ProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAiCompatible => "openai_compatible",
            ProviderKind::Anthropic => "anthropic",
        }
    }
}

/// A named endpoint the settings dropdown offers. Choosing one fills the
/// base URL and model, both of which stay editable for self-hosted servers.
#[derive(Debug, Clone, Copy)]
pub struct ProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ProviderKind,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub requires_key: bool,
}

/// The built-in catalogue. Add a vendor by adding one row.
pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "openai",
        label: "OpenAI",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o-mini",
        requires_key: true,
    },
    ProviderPreset {
        id: "deepseek",
        label: "DeepSeek",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.deepseek.com/v1",
        default_model: "deepseek-chat",
        requires_key: true,
    },
    ProviderPreset {
        id: "openrouter",
        label: "OpenRouter",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://openrouter.ai/api/v1",
        default_model: "anthropic/claude-3.5-sonnet",
        requires_key: true,
    },
    ProviderPreset {
        id: "groq",
        label: "Groq",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.groq.com/openai/v1",
        default_model: "llama-3.3-70b-versatile",
        requires_key: true,
    },
    ProviderPreset {
        id: "mistral",
        label: "Mistral",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.mistral.ai/v1",
        default_model: "mistral-small-latest",
        requires_key: true,
    },
    ProviderPreset {
        id: "together",
        label: "Together",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.together.xyz/v1",
        default_model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        requires_key: true,
    },
    ProviderPreset {
        id: "xai",
        label: "xAI",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.x.ai/v1",
        default_model: "grok-2-latest",
        requires_key: true,
    },
    ProviderPreset {
        id: "anthropic",
        label: "Anthropic",
        kind: ProviderKind::Anthropic,
        base_url: "https://api.anthropic.com/v1",
        default_model: "claude-3-5-sonnet-latest",
        requires_key: true,
    },
    ProviderPreset {
        id: "ollama",
        label: "Ollama (local)",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "http://localhost:11434/v1",
        default_model: "qwen2.5:7b",
        requires_key: false,
    },
    ProviderPreset {
        id: "lmstudio",
        label: "LM Studio (local)",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "http://localhost:1234/v1",
        default_model: "local-model",
        requires_key: false,
    },
    ProviderPreset {
        id: "custom",
        label: "Custom / self-hosted",
        kind: ProviderKind::OpenAiCompatible,
        base_url: "",
        default_model: "",
        requires_key: false,
    },
];

pub fn preset(id: &str) -> Option<&'static ProviderPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// LLM connection settings. The API key lives here and nowhere else; it is
/// written to the per-user config file, never to the repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmSettings {
    /// One of [`PRESETS`] ids.
    pub provider_id: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    /// Turns of history kept in context.
    pub context_turns: usize,
}

impl Default for LlmSettings {
    fn default() -> Self {
        let p = preset("deepseek").expect("deepseek preset");
        Self {
            provider_id: p.id.to_owned(),
            base_url: p.base_url.to_owned(),
            api_key: String::new(),
            model: p.default_model.to_owned(),
            temperature: 0.85,
            top_p: 0.95,
            max_tokens: 512,
            context_turns: 24,
        }
    }
}

impl LlmSettings {
    pub fn kind(&self) -> ProviderKind {
        preset(&self.provider_id)
            .map(|p| p.kind)
            .unwrap_or(ProviderKind::OpenAiCompatible)
    }

    /// Apply a preset, keeping any key the user already entered for that id.
    pub fn apply_preset(&mut self, id: &str) {
        if let Some(p) = preset(id) {
            self.provider_id = p.id.to_owned();
            self.base_url = p.base_url.to_owned();
            if self.model.trim().is_empty() || preset(&self.provider_id).is_none() {
                self.model = p.default_model.to_owned();
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    /// Speak replies aloud.
    pub enabled: bool,
    /// Platform voice identifier (`SpeechSynthesisVoice.voiceURI`). Empty means
    /// "pick the best installed voice that matches `preferred_locale`".
    pub voice_uri: String,
    pub preferred_locale: String,
    /// 0.5–2.0, 1.0 = natural.
    pub rate: f32,
    /// 0.0–2.0, 1.0 = natural. Slightly high reads as younger/anime.
    pub pitch: f32,
    /// 0.0–1.0.
    pub volume: f32,
    /// Start speaking as soon as a beat is complete.
    pub stream_speech: bool,
    /// Push-to-talk speech recognition.
    pub stt_enabled: bool,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_uri: String::new(),
            preferred_locale: "en".into(),
            rate: 1.04,
            pitch: 1.18,
            volume: 0.95,
            stream_speech: true,
            stt_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Default for Quality {
    fn default() -> Self {
        Quality::High
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AvatarSettings {
    /// File name inside `assets/models/`. Empty resolves to the first VRM found.
    pub model_file: String,
    pub scale: f32,
    /// Head/eye tracking toward the cursor.
    pub gaze_follow: bool,
    pub auto_blink: bool,
    pub idle_motion: bool,
    /// Mouth movement strength.
    pub lipsync_gain: f32,
    pub quality: Quality,
}

impl Default for AvatarSettings {
    fn default() -> Self {
        Self {
            model_file: String::new(),
            scale: 1.0,
            gaze_follow: true,
            auto_blink: true,
            idle_motion: true,
            lipsync_gain: 1.0,
            quality: Quality::High,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub always_on_top: bool,
    /// 0.6–1.0 window opacity.
    pub window_opacity: f32,
    /// Accent hue in degrees; drives the whole palette.
    pub accent_hue: f32,
    pub show_transcript: bool,
    pub reduce_motion: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            window_opacity: 0.98,
            accent_hue: 340.0,
            show_transcript: true,
            reduce_motion: false,
        }
    }
}

/// The whole persisted state, versioned.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub persona: Persona,
    pub llm: LlmSettings,
    pub voice: VoiceSettings,
    pub avatar: AvatarSettings,
    pub ui: UiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            persona: Persona::default(),
            llm: LlmSettings::default(),
            voice: VoiceSettings::default(),
            avatar: AvatarSettings::default(),
            ui: UiSettings::default(),
        }
    }
}

impl Settings {
    /// Clamp anything a hand-edited config could get wrong, and back-fill
    /// fields added in later versions. Always safe to call.
    pub fn normalize(&mut self) {
        if self.version > SETTINGS_VERSION {
            self.version = SETTINGS_VERSION;
        }
        self.version = SETTINGS_VERSION;

        if preset(&self.llm.provider_id).is_none() {
            self.llm.apply_preset("custom");
        }
        self.llm.base_url = self.llm.base_url.trim().trim_end_matches('/').to_owned();
        self.llm.model = self.llm.model.trim().to_owned();
        self.llm.temperature = self.llm.temperature.clamp(0.0, 2.0);
        self.llm.top_p = self.llm.top_p.clamp(0.0, 1.0);
        self.llm.max_tokens = self.llm.max_tokens.clamp(16, 8192);
        self.llm.context_turns = self.llm.context_turns.clamp(0, 200);

        self.voice.rate = self.voice.rate.clamp(0.5, 2.0);
        self.voice.pitch = self.voice.pitch.clamp(0.0, 2.0);
        self.voice.volume = self.voice.volume.clamp(0.0, 1.0);

        self.avatar.scale = self.avatar.scale.clamp(0.3, 2.5);
        self.avatar.lipsync_gain = self.avatar.lipsync_gain.clamp(0.0, 2.0);

        self.ui.window_opacity = self.ui.window_opacity.clamp(0.6, 1.0);
        self.ui.accent_hue = self.ui.accent_hue.rem_euclid(360.0);

        if self.persona.name.trim().is_empty() {
            self.persona.name = Persona::default().name;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_normalize_is_idempotent() {
        let mut s = Settings::default();
        s.normalize();
        let once = serde_json::to_string(&s).unwrap();
        s.normalize();
        assert_eq!(once, serde_json::to_string(&s).unwrap());
        assert_eq!(s.version, SETTINGS_VERSION);
    }

    #[test]
    fn clamps_hand_edited_values() {
        let mut s = Settings::default();
        s.llm.temperature = 9.0;
        s.voice.rate = 0.0;
        s.ui.accent_hue = 720.0;
        s.normalize();
        assert_eq!(s.llm.temperature, 2.0);
        assert_eq!(s.voice.rate, 0.5);
        assert_eq!(s.ui.accent_hue, 0.0);
    }

    #[test]
    fn unknown_provider_falls_back_to_custom() {
        let mut s = Settings::default();
        s.llm.provider_id = "nope".into();
        s.normalize();
        assert_eq!(s.llm.provider_id, "custom");
    }

    #[test]
    fn every_preset_id_is_unique_and_resolvable() {
        for (i, p) in PRESETS.iter().enumerate() {
            assert_eq!(preset(p.id).map(|x| x.id), Some(p.id));
            assert!(
                !PRESETS[..i].iter().any(|q| q.id == p.id),
                "duplicate preset id {}",
                p.id
            );
        }
    }

    #[test]
    fn defaults_round_trip_through_json() {
        let s = Settings::default();
        let json = serde_json::to_string_pretty(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.llm.provider_id, s.llm.provider_id);
        assert_eq!(back.persona.name, "Yura");
    }
}
