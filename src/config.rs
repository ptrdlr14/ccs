use std::collections::{BTreeMap, HashMap};
use std::env;

use serde::{Deserialize, Serialize};

use crate::fatal;

// ── types ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Clone)]
pub struct Provider {
    pub base_url: String,
    pub env_key: String,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Models {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_fable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_opus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_sonnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_haiku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Default)]
pub struct Profile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Models>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    /// Arbitrary environment variables injected verbatim.
    /// Applied last, so they win over `models` / `provider` on key conflicts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub profiles: HashMap<String, Profile>,
}

// ── config loading ─────────────────────────────────────────────────────────

const DEFAULT_CONFIG: &str = r#"# ccs configuration file
#
# Example:
# [profiles.my-provider]
# description = "My API provider"
#
# [profiles.my-provider.provider]
# base_url = "https://api.example.com"
# env_key = "MY_API_KEY"
#
# [profiles.my-provider.models]
# default = "claude-sonnet-4-6"
#
# [profiles.my-provider.env]
# CLAUDE_CODE_MAX_OUTPUT_TOKENS = "16000"

[profiles]
"#;

fn config_path() -> String {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .expect("cannot determine home directory");
    format!("{home}/.config/ccs/config.toml")
}

fn ensure_config_dir(cpath: &str) {
    if let Some(parent) = std::path::Path::new(cpath).parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| fatal(&format!("cannot create config directory: {e}")));
    }
}

pub fn load_config() -> Config {
    let cpath = config_path();
    let content = match std::fs::read_to_string(&cpath) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            ensure_config_dir(&cpath);
            std::fs::write(&cpath, DEFAULT_CONFIG)
                .unwrap_or_else(|e| fatal(&format!("cannot write default config to {cpath}: {e}")));
            return Config {
                profiles: HashMap::new(),
            };
        }
        Err(e) => fatal(&format!("cannot read {cpath}: {e}")),
    };
    let config: Config = toml::from_str(&content)
        .unwrap_or_else(|e| fatal(&format!("invalid config in {cpath}: {e}")));
    config
}

// ── environment ────────────────────────────────────────────────────────────

pub fn build_env(profile: &Profile, reveal: bool) -> HashMap<String, String> {
    let mut env_map = HashMap::new();

    if let Some(ref models) = profile.models {
        let pairs: [(&str, &Option<String>); 6] = [
            ("ANTHROPIC_MODEL", &models.default),
            ("ANTHROPIC_DEFAULT_FABLE_MODEL", &models.default_fable),
            ("ANTHROPIC_DEFAULT_OPUS_MODEL", &models.default_opus),
            ("ANTHROPIC_DEFAULT_SONNET_MODEL", &models.default_sonnet),
            ("ANTHROPIC_DEFAULT_HAIKU_MODEL", &models.default_haiku),
            ("CLAUDE_CODE_SUBAGENT_MODEL", &models.subagent),
        ];
        for (key, val) in pairs {
            if let Some(ref v) = val {
                env_map.insert(key.to_string(), v.clone());
            }
        }
    }

    if let Some(ref provider) = profile.provider {
        env_map.insert("ANTHROPIC_BASE_URL".into(), provider.base_url.clone());
        if reveal {
            let token = env::var(&provider.env_key).unwrap_or_else(|_| {
                fatal(&format!(
                    "environment variable {} is not set",
                    provider.env_key
                ));
            });
            env_map.insert("ANTHROPIC_AUTH_TOKEN".into(), token);
            env_map.insert("ANTHROPIC_API_KEY".into(), String::new());
        } else {
            env_map.insert(
                "ANTHROPIC_AUTH_TOKEN".into(),
                format!("${}", provider.env_key),
            );
            env_map.insert("ANTHROPIC_API_KEY".into(), "(cleared)".into());
        }
    }

    // keep last: the env table wins over models/provider (see Profile::env).
    // Correctness here depends on source order — nothing may be appended below.
    if let Some(ref env) = profile.env {
        env_map.extend(env.clone());
    }

    env_map
}

// ── save ───────────────────────────────────────────────────────────────────

pub fn save_config(config: &Config) {
    let cpath = config_path();
    ensure_config_dir(&cpath);
    let content = toml::to_string_pretty(config)
        .unwrap_or_else(|e| fatal(&format!("failed to serialize config: {e}")));
    std::fs::write(&cpath, content)
        .unwrap_or_else(|e| fatal(&format!("cannot write {cpath}: {e}")));
}

// ── field definitions ──────────────────────────────────────────────────────

pub struct FieldDef {
    pub label: &'static str,
    pub section: &'static str,
    pub get: fn(&Profile) -> Option<String>,
    pub set: fn(&mut Profile, String),
}

fn none_if_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

macro_rules! model_field {
    ($get:ident, $set:ident, $field:ident) => {
        fn $get(p: &Profile) -> Option<String> {
            p.models.as_ref()?.$field.clone()
        }
        fn $set(p: &mut Profile, v: String) {
            p.models.get_or_insert_default().$field = none_if_empty(v);
        }
    };
}

// Profile section
fn get_description(p: &Profile) -> Option<String> {
    p.description.clone()
}
fn set_description(p: &mut Profile, v: String) {
    p.description = none_if_empty(v);
}

// Models section
model_field!(get_default, set_default, default);
model_field!(get_default_fable, set_default_fable, default_fable);
model_field!(get_default_opus, set_default_opus, default_opus);
model_field!(get_default_sonnet, set_default_sonnet, default_sonnet);
model_field!(get_default_haiku, set_default_haiku, default_haiku);
model_field!(get_subagent, set_subagent, subagent);

// Provider section
fn get_base_url(p: &Profile) -> Option<String> {
    Some(p.provider.as_ref()?.base_url.clone())
}
fn set_base_url(p: &mut Profile, v: String) {
    if v.is_empty() {
        return;
    }
    let pr = match p.provider {
        Some(ref mut pr) => pr,
        None => {
            p.provider = Some(Provider {
                base_url: String::new(),
                env_key: String::new(),
            });
            p.provider.as_mut().unwrap()
        }
    };
    pr.base_url = v;
}

fn get_env_key(p: &Profile) -> Option<String> {
    p.provider.as_ref().map(|pr| pr.env_key.clone())
}
fn set_env_key(p: &mut Profile, v: String) {
    let pr = match p.provider {
        Some(ref mut pr) => pr,
        None => {
            p.provider = Some(Provider {
                base_url: String::new(),
                env_key: String::new(),
            });
            p.provider.as_mut().unwrap()
        }
    };
    pr.env_key = v;
}

pub const PROFILE_FIELDS: &[FieldDef] = &[
    FieldDef {
        label: "description",
        section: "Profile",
        get: get_description,
        set: set_description,
    },
    FieldDef {
        label: "default",
        section: "Models",
        get: get_default,
        set: set_default,
    },
    FieldDef {
        label: "fable",
        section: "Models",
        get: get_default_fable,
        set: set_default_fable,
    },
    FieldDef {
        label: "opus",
        section: "Models",
        get: get_default_opus,
        set: set_default_opus,
    },
    FieldDef {
        label: "sonnet",
        section: "Models",
        get: get_default_sonnet,
        set: set_default_sonnet,
    },
    FieldDef {
        label: "haiku",
        section: "Models",
        get: get_default_haiku,
        set: set_default_haiku,
    },
    FieldDef {
        label: "subagent",
        section: "Models",
        get: get_subagent,
        set: set_subagent,
    },
    FieldDef {
        label: "base_url",
        section: "Provider",
        get: get_base_url,
        set: set_base_url,
    },
    FieldDef {
        label: "env_key",
        section: "Provider",
        get: get_env_key,
        set: set_env_key,
    },
];

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_table_is_injected() {
        let profile = Profile {
            env: Some(BTreeMap::from([
                (
                    "CLAUDE_CODE_MAX_OUTPUT_TOKENS".to_string(),
                    "16000".to_string(),
                ),
                (
                    "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE".to_string(),
                    "80".to_string(),
                ),
            ])),
            ..Default::default()
        };

        let e = build_env(&profile, true);
        assert_eq!(
            e.get("CLAUDE_CODE_MAX_OUTPUT_TOKENS").map(String::as_str),
            Some("16000")
        );
        assert_eq!(
            e.get("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE").map(String::as_str),
            Some("80")
        );
    }

    #[test]
    fn env_table_overrides_models_on_conflict() {
        let profile = Profile {
            models: Some(Models {
                default: Some("base-model".to_string()),
                ..Default::default()
            }),
            env: Some(BTreeMap::from([(
                "ANTHROPIC_MODEL".to_string(),
                "override-model".to_string(),
            )])),
            ..Default::default()
        };

        let e = build_env(&profile, true);
        assert_eq!(
            e.get("ANTHROPIC_MODEL").map(String::as_str),
            Some("override-model")
        );
    }

    #[test]
    fn env_table_overrides_provider_on_conflict() {
        let profile = Profile {
            provider: Some(Provider {
                base_url: "https://api.example.com".to_string(),
                env_key: "MY_API_KEY".to_string(),
            }),
            env: Some(BTreeMap::from([(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://override.example.com".to_string(),
            )])),
            ..Default::default()
        };

        // reveal = false so no ambient environment variable is read
        let e = build_env(&profile, false);
        assert_eq!(
            e.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://override.example.com")
        );
    }

    #[test]
    fn env_table_round_trips() {
        let toml_src = r#"
[profiles.demo]
[profiles.demo.env]
CLAUDE_CODE_MAX_CONTEXT_TOKENS = "128000"
"#;
        let config: Config = toml::from_str(toml_src).unwrap();
        let profile = config.profiles.get("demo").unwrap();
        assert_eq!(
            profile
                .env
                .as_ref()
                .unwrap()
                .get("CLAUDE_CODE_MAX_CONTEXT_TOKENS")
                .map(String::as_str),
            Some("128000")
        );

        // re-serialize keeps the env table
        let out = toml::to_string_pretty(&config).unwrap();
        let reparsed: Config = toml::from_str(&out).unwrap();
        assert!(reparsed.profiles.get("demo").unwrap().env.is_some());
    }
}
