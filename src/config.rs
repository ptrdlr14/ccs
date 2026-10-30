use std::collections::{BTreeMap, HashMap};
use std::env;
use std::io::Write;

use serde::Deserialize;

use crate::fatal;

// ── types ──────────────────────────────────────────────────────────────────

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    String(String),
    Flag(bool),
    List(Vec<String>),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvSource {
    pub from_env: String,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    Literal(String),
    FromEnv(EnvSource),
}

#[derive(Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub args: BTreeMap<String, ArgValue>,
    #[serde(default)]
    pub env: BTreeMap<String, EnvValue>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub description: Option<String>,
    #[serde(default)]
    pub uses: Vec<String>,
    #[serde(default)]
    pub args: BTreeMap<String, ArgValue>,
    #[serde(default)]
    pub env: BTreeMap<String, EnvValue>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub shared: HashMap<String, Settings>,
    pub profiles: HashMap<String, Profile>,
}

impl Config {
    pub fn resolve(&self, profile: &Profile, profile_name: &str) -> Result<Settings, String> {
        let mut settings = Settings::default();
        for name in &profile.uses {
            let shared = self.shared.get(name).ok_or_else(|| {
                format!("profile \"{profile_name}\": unknown shared configuration \"{name}\"")
            })?;
            settings.args.extend(shared.args.clone());
            settings.env.extend(shared.env.clone());
        }
        settings.args.extend(profile.args.clone());
        settings.env.extend(profile.env.clone());
        Ok(settings)
    }
}

impl Settings {
    pub fn build_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (name, value) in &self.args {
            match value {
                ArgValue::String(value) => args.push(format!("--{name}={value}")),
                ArgValue::Flag(true) => args.push(format!("--{name}")),
                ArgValue::Flag(false) => {}
                ArgValue::List(values) => {
                    args.extend(values.iter().map(|value| format!("--{name}={value}")));
                }
            }
        }
        args
    }

    pub fn build_env(&self, profile_name: &str) -> Result<HashMap<String, String>, String> {
        self.env
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    EnvValue::Literal(value) => value.clone(),
                    EnvValue::FromEnv(source) => env::var(&source.from_env).map_err(|e| {
                        format!(
                            "profile \"{profile_name}\", env.{key}: cannot read environment variable {}: {e}",
                            source.from_env
                        )
                    })?,
                };
                Ok((key.clone(), value))
            })
            .collect()
    }
}

// ── config loading ─────────────────────────────────────────────────────────

const DEFAULT_CONFIG: &str = r#"# ccs configuration file
#
# Example:
# [shared.common.args]
# permission-mode = "plan"
#
# [shared.proxy.env]
# ANTHROPIC_BASE_URL = "https://api.example.com"
# ANTHROPIC_AUTH_TOKEN = { from_env = "MY_API_KEY" }
# ANTHROPIC_API_KEY = ""
#
# [profiles.coding]
# description = "Coding"
# uses = ["common", "proxy"]
#
# [profiles.coding.args]
# plugin-dir = ["./plugins/coding", "./plugins/tools"]
#
# [profiles.coding.env]
# ANTHROPIC_MODEL = "my-model"
# CLAUDE_CODE_MAX_OUTPUT_TOKENS = "16000"

[profiles]
"#;

fn config_path() -> String {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .expect("cannot determine home directory");
    format!("{home}/.config/ccs/config.toml")
}

fn create_default_config(cpath: &std::path::Path) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(cpath)?;
    file.write_all(DEFAULT_CONFIG.as_bytes())
}

pub fn load_config() -> Config {
    let cpath = config_path();
    let content = match std::fs::read_to_string(&cpath) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = std::path::Path::new(&cpath).parent() {
                std::fs::create_dir_all(parent)
                    .unwrap_or_else(|e| fatal(&format!("cannot create config directory: {e}")));
            }
            match create_default_config(std::path::Path::new(&cpath)) {
                Ok(()) => return Config::default(),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::fs::read_to_string(&cpath)
                        .unwrap_or_else(|e| fatal(&format!("cannot read {cpath}: {e}")))
                }
                Err(e) => fatal(&format!("cannot write default config to {cpath}: {e}")),
            }
        }
        Err(e) => fatal(&format!("cannot read {cpath}: {e}")),
    };
    toml::from_str(&content).unwrap_or_else(|e| fatal(&format!("invalid config in {cpath}: {e}")))
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_creation_never_overwrites_existing_content() {
        let dir = std::env::temp_dir().join(format!("ccs-create-test-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("config.toml");
        create_default_config(&path).unwrap();
        let created = std::fs::read_to_string(&path).unwrap();

        let existing = "[profiles.personal]\ndescription = 'Keep this configuration'\n";
        std::fs::write(&path, existing).unwrap();
        let result = create_default_config(&path);
        let preserved = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(created, DEFAULT_CONFIG);
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(preserved, existing);
    }

    #[test]
    fn empty_profile_has_no_args_or_env() {
        let config: Config = toml::from_str("[profiles.official]\n").unwrap();
        let settings = config
            .resolve(&config.profiles["official"], "official")
            .unwrap();
        assert!(settings.build_args().is_empty());
        assert!(settings.build_env("official").unwrap().is_empty());
    }

    #[test]
    fn shared_settings_merge_in_order_then_profile_overrides() {
        let config: Config = toml::from_str(
            r#"
[shared.first.args]
permission-mode = "plan"
plugin-dir = ["first", "second"]
verbose = true
[shared.first.env]
TOKEN = { from_env = "UNUSED_MISSING_TOKEN" }
FIRST = "first"
ORDER = "first"
[shared.second.args]
permission-mode = "auto"
plugin-dir = ["third"]
[shared.second.env]
ORDER = "second"
[profiles.demo]
uses = ["first", "second"]
[profiles.demo.args]
plugin-dir = ["final", "final"]
[profiles.demo.env]
TOKEN = "overridden"
"#,
        )
        .unwrap();
        let settings = config.resolve(&config.profiles["demo"], "demo").unwrap();
        assert_eq!(
            settings.build_args(),
            [
                "--permission-mode=auto",
                "--plugin-dir=final",
                "--plugin-dir=final",
                "--verbose"
            ]
        );
        let env = settings.build_env("demo").unwrap();
        assert_eq!(env["FIRST"], "first");
        assert_eq!(env["ORDER"], "second");
        assert_eq!(env["TOKEN"], "overridden");
    }

    #[test]
    fn arg_values_preserve_literals_and_allow_clearing_shared_values() {
        let config: Config = toml::from_str(
            r#"
[shared.common.args]
plugin-dir = ["inherited"]
verbose = true
[profiles.demo]
uses = ["common"]
[profiles.demo.args]
plugin-dir = []
verbose = false
append-system-prompt = "${HOME} $HOME ~/*.txt"
model = ""
[profiles.demo.env]
EMPTY = ""
LITERAL = "${HOME} $HOME ~/*.txt"
"#,
        )
        .unwrap();
        let settings = config.resolve(&config.profiles["demo"], "demo").unwrap();
        assert_eq!(
            settings.build_args(),
            ["--append-system-prompt=${HOME} $HOME ~/*.txt", "--model="]
        );
        let env = settings.build_env("demo").unwrap();
        assert_eq!(env["EMPTY"], "");
        assert_eq!(env["LITERAL"], "${HOME} $HOME ~/*.txt");
    }

    #[test]
    fn missing_shared_configuration_identifies_profile_and_reference() {
        let config: Config = toml::from_str("[profiles.demo]\nuses = ['missing']\n").unwrap();
        let error = config
            .resolve(&config.profiles["demo"], "demo")
            .err()
            .unwrap();
        assert_eq!(
            error,
            "profile \"demo\": unknown shared configuration \"missing\""
        );
    }

    #[test]
    fn default_config_is_valid() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(config.profiles.is_empty());
        let example = DEFAULT_CONFIG
            .lines()
            .skip(3)
            .map(|line| line.strip_prefix("# ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        let config: Config = toml::from_str(&example).unwrap();
        let settings = config
            .resolve(&config.profiles["coding"], "coding")
            .unwrap();
        assert_eq!(
            settings.build_args(),
            [
                "--permission-mode=plan",
                "--plugin-dir=./plugins/coding",
                "--plugin-dir=./plugins/tools"
            ]
        );
        assert!(matches!(
            settings.env["ANTHROPIC_AUTH_TOKEN"],
            EnvValue::FromEnv(_)
        ));
    }

    #[test]
    fn invalid_fields_and_types_are_rejected() {
        for source in [
            "unknown = true\n[profiles]\n",
            "[profiles.demo.models]\ndefault = 'old-model'\n",
            "[profiles.demo.provider]\nbase_url = 'https://api.example.com'\nenv_key = 'KEY'\n",
            "[profiles.demo]\narg = ['--help']\n",
            "[profiles.demo]\nargs = ['--help']\n",
            "[profiles.demo]\nuses = 'common'\n",
            "[profiles.demo.args]\nmodel = 42\n",
            "[profiles.demo.args]\nplugin-dir = ['a', true]\n",
            "[shared.common]\nuses = ['other']\n[profiles]\n",
            "[profiles.demo.env]\nKEY = 42\n",
            "[profiles.demo.env]\nKEY = { from_env = 'KEY', typo = true }\n",
            "[profiles.demo.env]\nKEY = { from_en = 'KEY' }\n",
        ] {
            assert!(toml::from_str::<Config>(source).is_err(), "{source}");
        }
    }
}
