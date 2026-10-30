use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;

mod common;

use common::Fixture;

#[test]
fn help_does_not_create_or_load_config() {
    let fixture = Fixture::new();
    let output = fixture.command().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("ccs [OPTIONS] [PROFILE]"));
    assert!(!fixture.root.join(".config").exists());

    fixture.config("invalid toml");
    assert!(fixture.command().arg("-h").status().unwrap().success());
}

#[test]
fn version_does_not_create_or_load_config() {
    let fixture = Fixture::new();
    for argument in ["--version", "-V"] {
        let output = fixture.command().arg(argument).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            concat!("ccs ", env!("CARGO_PKG_VERSION"))
        );
    }
    assert!(!fixture.root.join(".config").exists());

    fixture.config("invalid toml");
    assert!(fixture
        .command()
        .arg("--version")
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn profile_arguments_are_forwarded_without_parsing() {
    let fixture = Fixture::new();
    fixture.config(
        "[shared.common.args]\npermission-mode = 'plan'\nadd-dir = ['/shared projects', '$HOME/*.txt']\n[profiles.demo]\nuses = ['common']\n[profiles.help]\nuses = ['common']\n",
    );
    let claude = fixture.root.join("claude");
    fs::write(&claude, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();

    for profile in ["demo", "help"] {
        for arguments in [
            vec!["--help"],
            vec!["--version"],
            vec!["--", "--help", "demo"],
            vec!["-h", "-V", "", "a,b"],
        ] {
            let output = fixture
                .command()
                .env("PATH", &fixture.root)
                .arg(profile)
                .args(&arguments)
                .output()
                .unwrap();
            assert!(output.status.success(), "{:?}", output);
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                format!(
                    "--add-dir=/shared projects\n--add-dir=$HOME/*.txt\n--permission-mode=plan\n{}\n",
                    arguments.join("\n")
                )
            );
        }
    }
}

#[test]
fn no_arguments_create_config_and_list_profiles_in_order() {
    let fixture = Fixture::new();
    let output = fixture.command().output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No profiles configured"));
    assert!(fixture.root.join(".config/ccs/config.toml").exists());

    fixture.config(
        "[shared.common.env]\nTOKEN = { from_env = 'CCS_TEST_MISSING_TOKEN' }\n[profiles.zulu]\nuses = ['common']\n[profiles.alpha]\ndescription = 'First'\n",
    );
    let output = fixture.command().output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).ends_with("  alpha  First\n  zulu\n"));
}

#[test]
fn launch_preserves_arguments_environment_and_exit_status() {
    let fixture = Fixture::new();
    fixture.config(
        r#"
[shared.common.args]
permission-mode = "auto"
[shared.common.env]
ANTHROPIC_MODEL = "shared-model"
CCS_TEST_CUSTOM = "shared-value"
[shared.proxy.env]
ANTHROPIC_BASE_URL = "https://api.example.com"
ANTHROPIC_AUTH_TOKEN = { from_env = "CCS_TEST_TOKEN" }
ANTHROPIC_API_KEY = ""
[profiles.demo]
uses = ["common", "proxy"]
[profiles.demo.args]
permission-mode = "plan"
[profiles.demo.env]
ANTHROPIC_MODEL = "override-model"
CCS_TEST_CUSTOM = "profile-value"
CCS_TEST_COPIED = { from_env = "CCS_TEST_CUSTOM" }
CCS_TEST_ORIGINAL_MODEL = { from_env = "ANTHROPIC_MODEL" }
"#,
    );
    let claude = fixture.root.join("claude");
    fs::write(
        &claude,
        r#"#!/bin/sh
printf '%s\n' "$@"
printf 'model=%s\nurl=%s\ntoken=%s\napi_key=%s\nstale=%s\ncustom=%s\ncopied=%s\noriginal_model=%s\n' "$ANTHROPIC_MODEL" "$ANTHROPIC_BASE_URL" "$ANTHROPIC_AUTH_TOKEN" "$ANTHROPIC_API_KEY" "${ANTHROPIC_CCS_TEST_STALE-unset}" "$CCS_TEST_CUSTOM" "$CCS_TEST_COPIED" "$CCS_TEST_ORIGINAL_MODEL"
exit 23
"#,
    )
    .unwrap();
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();

    let output = fixture
        .command()
        .env("PATH", &fixture.root)
        .env("CCS_TEST_TOKEN", "test-token")
        .env("ANTHROPIC_MODEL", "shell-model")
        .env("ANTHROPIC_API_KEY", "old-key")
        .env("ANTHROPIC_CCS_TEST_STALE", "old-value")
        .env("CCS_TEST_CUSTOM", "shell-value")
        .args(["demo", "-p", "demo", "hello world", "你好", "--help"])
        .arg(std::ffi::OsStr::from_bytes(b"non-utf8-\xff"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(23));
    let mut expected = "--permission-mode=plan\n-p\ndemo\nhello world\n你好\n--help\nnon-utf8-"
        .as_bytes()
        .to_vec();
    expected.extend_from_slice(b"\xff\nmodel=override-model\nurl=https://api.example.com\ntoken=test-token\napi_key=\nstale=unset\ncustom=profile-value\ncopied=shell-value\noriginal_model=shell-model\n");
    assert_eq!(output.stdout, expected);
}

#[test]
fn unknown_profiles_and_options_fail() {
    let fixture = Fixture::new();
    fixture.config("[profiles]\n");
    for (argument, message) in [
        ("missing", "unknown profile \"missing\""),
        ("--bad-option", "unexpected argument '--bad-option'"),
    ] {
        let output = fixture.command().arg(argument).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "{stderr}");
        assert!(stderr.contains("Usage:"));
    }
}

#[test]
fn shared_configurations_are_not_launchable_profiles() {
    let fixture = Fixture::new();
    fixture.config("[shared.common.args]\nverbose = true\n[profiles.demo]\nuses = ['missing']\n");
    let output = fixture.command().arg("common").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown profile \"common\""));

    let output = fixture.command().arg("demo").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("profile \"demo\": unknown shared configuration \"missing\""));
}

#[test]
fn missing_env_source_fails_before_launch() {
    let fixture = Fixture::new();
    fixture.config(
        "[profiles.demo.env]\nANTHROPIC_AUTH_TOKEN = { from_env = 'CCS_TEST_MISSING_TOKEN' }\n",
    );
    let output = fixture
        .command()
        .env_remove("CCS_TEST_MISSING_TOKEN")
        .arg("demo")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("profile \"demo\", env.ANTHROPIC_AUTH_TOKEN"));
    assert!(stderr.contains("cannot read environment variable CCS_TEST_MISSING_TOKEN"));
}
