use yura_core::{preset, LlmSettings, ProviderKind, Result, YuraError};

use crate::anthropic::Anthropic;
use crate::openai::OpenAiCompatible;
use crate::provider::LlmProvider;

/// Build the backend described by `cfg`.
///
/// Fails fast and locally when a key is missing, so the UI can say
/// "add your API key" instead of surfacing a 401 from the network.
pub fn build(cfg: &LlmSettings) -> Result<Box<dyn LlmProvider>> {
    let entry = preset(&cfg.provider_id);
    let requires_key = entry.map(|p| p.requires_key).unwrap_or(false);

    if requires_key && cfg.api_key.trim().is_empty() {
        return Err(YuraError::MissingApiKey(cfg.provider_id.clone()));
    }

    let base_url = if cfg.base_url.trim().is_empty() {
        entry.map(|p| p.base_url).unwrap_or_default().to_owned()
    } else {
        cfg.base_url.trim().to_owned()
    };

    match cfg.kind() {
        ProviderKind::Anthropic => Ok(Box::new(Anthropic::new(base_url, cfg.api_key.clone()))),
        ProviderKind::OpenAiCompatible => {
            Ok(Box::new(OpenAiCompatible::new(base_url, cfg.api_key.clone())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(id: &str, key: &str) -> LlmSettings {
        let mut s = LlmSettings::default();
        s.provider_id = id.into();
        s.api_key = key.into();
        s
    }

    #[test]
    fn rejects_cloud_provider_without_key() {
        assert!(matches!(
            build(&cfg("openai", "")),
            Err(YuraError::MissingApiKey(_))
        ));
    }

    #[test]
    fn local_providers_need_no_key() {
        assert!(build(&cfg("ollama", "")).is_ok());
        assert!(build(&cfg("lmstudio", "")).is_ok());
    }

    #[test]
    fn picks_the_right_backend() {
        assert_eq!(build(&cfg("openai", "sk")).unwrap().id(), "openai_compatible");
        assert_eq!(build(&cfg("anthropic", "sk")).unwrap().id(), "anthropic");
    }
}
