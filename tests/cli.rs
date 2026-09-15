use std::{fs, process::Command};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_herdr-progress"))
}

#[test]
fn instruction_aliases_are_read_only_and_identical() {
    let dir = tempfile::tempdir().unwrap();
    let mut outputs = Vec::new();
    for flag in ["--instructions", "--skill"] {
        let result = cli()
            .arg(flag)
            .env("HERDR_PLUGIN_CONFIG_DIR", dir.path().join("config"))
            .env("HERDR_PLUGIN_STATE_DIR", dir.path().join("state"))
            .output()
            .unwrap();
        assert!(result.status.success());
        outputs.push(result.stdout);
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn hooks_are_inert_outside_herdr_even_with_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli()
        .args(["hook", "--agent", "codex"])
        .env_remove("HERDR_ENV")
        .env("HERDR_PLUGIN_CONFIG_DIR", dir.path().join("config"))
        .env("HERDR_PLUGIN_STATE_DIR", dir.path().join("state"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn cli_rejects_invalid_estimates_and_ambiguous_forms() {
    for args in [
        vec![
            "report",
            "--binding",
            "B",
            "--task",
            "T",
            "--percent",
            "101",
            "--activity",
            "Testing changes",
        ],
        vec![
            "report",
            "--binding",
            "B",
            "--task",
            "T",
            "--percent",
            "50",
            "--unknown",
            "--activity",
            "Testing changes",
        ],
        vec![
            "begin",
            "--binding",
            "B",
            "--title",
            "Missing expected generation",
        ],
        vec!["context"],
    ] {
        assert!(!cli().args(args).output().unwrap().status.success());
    }
}
