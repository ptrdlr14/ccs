use std::fs;
use std::os::unix::fs::PermissionsExt;

mod common;

use common::Fixture;

#[test]
fn empty_env_reference_and_profile_args_work_without_cli_args() {
    let fixture = Fixture::new();
    fixture.config(
        r#"
[shared.common.args]
plugin-dir = ["inherited"]
verbose = true
[shared.common.env]
CCS_TEST_EMPTY = { from_env = "CCS_TEST_SOURCE" }
CCS_TEST_VALUE = { from_env = "CCS_TEST_UNUSED_MISSING" }
[profiles.demo]
uses = ["common"]
[profiles.demo.args]
append-system-prompt = "hello world"
model = ""
plugin-dir = ["${HOME}", "${HOME}"]
[profiles.demo.env]
CCS_TEST_VALUE = "overridden"
"#,
    );
    let claude = fixture.root.join("claude");
    fs::write(
        &claude,
        "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\nprintf 'empty=<%s>\\nvalue=<%s>\\n' \"${CCS_TEST_EMPTY-unset}\" \"$CCS_TEST_VALUE\"\n",
    )
    .unwrap();
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
    let output = fixture
        .command()
        .env("PATH", &fixture.root)
        .env("CCS_TEST_SOURCE", "")
        .arg("demo")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        b"<--append-system-prompt=hello world>\n<--model=>\n<--plugin-dir=${HOME}>\n<--plugin-dir=${HOME}>\n<--verbose>\nempty=<>\nvalue=<overridden>\n"
    );
}
