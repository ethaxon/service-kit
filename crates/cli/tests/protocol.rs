use serde_json::{Value, json};
use std::process::Command;
fn call(args: &[&str]) -> (bool, Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_service-kit"))
        .args(args)
        .output()
        .unwrap();
    (
        o.status.success(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}
#[test]
fn invalid_arguments_have_envelope() {
    let (ok, v) = call(&["unknown"]);
    assert!(!ok);
    assert_eq!(v["protocolVersion"], 1);
    assert_eq!(v["error"]["code"], "INVALID_ARGUMENTS");
}
#[test]
fn offline_install_and_invalid_spec() {
    let t = test_directory();
    let p = t.path().join("spec.json");
    std::fs::write(&p,serde_json::to_vec(&json!({"id":"test", "executable":"/bin/echo", "scope":"system","startup":"manual","restart":"never"})).unwrap()).unwrap();
    let p = p.to_str().unwrap();
    let (ok, v) = call(&["plan", "install", "--spec", p, "--backend", "systemd"]);
    assert!(ok, "{v}");
    assert_eq!(v["data"]["backend"], "systemd");
    assert_eq!(v["data"]["actions"][0]["type"], "writeFile");
    std::fs::write(p, b"{}").unwrap();
    let (ok, v) = call(&["install", "--spec", p, "--backend", "systemd", "--dry-run"]);
    assert!(!ok);
    assert_eq!(v["error"]["code"], "INVALID_SPEC");
}

fn test_directory() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../temp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

#[test]
fn configuration_providers_produce_equivalent_plans() {
    let dir = test_directory();
    let sources = [
        (
            "json",
            r#"{"id":"test","executable":"/bin/echo","scope":"system","startup":"manual","restart":"never","args":["hello world"],"environment":{"MODE":"test"}}"#,
        ),
        (
            "toml",
            "id = 'test'\nexecutable = '/bin/echo'\nscope = 'system'\nstartup = 'manual'\nrestart = 'never'\nargs = ['hello world']\n[environment]\nMODE = 'test'\n",
        ),
        (
            "yaml",
            "id: test\nexecutable: /bin/echo\nscope: system\nstartup: manual\nrestart: never\nargs: ['hello world']\nenvironment:\n  MODE: test\n",
        ),
        (
            "yml",
            "id: test\nexecutable: /bin/echo\nscope: system\nstartup: manual\nrestart: never\nargs: ['hello world']\nenvironment:\n  MODE: test\n",
        ),
    ];
    let mut expected = None;
    for (extension, source) in sources {
        let path = dir.path().join(format!("spec.{extension}"));
        std::fs::write(&path, source).unwrap();
        let (ok, result) = call(&[
            "plan",
            "install",
            "--spec",
            path.to_str().unwrap(),
            "--backend",
            "systemd",
        ]);
        assert!(ok, "{extension}: {result}");
        if let Some(expected) = &expected {
            assert_eq!(&result, expected);
        } else {
            expected = Some(result);
        }
    }
    for (name, code) in [("missing.json", "IO"), ("spec.txt", "INVALID_SPEC")] {
        let path = dir.path().join(name);
        let (ok, result) = call(&[
            "plan",
            "install",
            "--spec",
            path.to_str().unwrap(),
            "--backend",
            "systemd",
        ]);
        assert!(!ok);
        assert_eq!(result["error"]["code"], code);
    }
}

#[test]
fn conflicting_options_and_camel_case_plan_fields() {
    let dir = test_directory();
    let path = dir.path().join("spec.json");
    std::fs::write(&path, r#"{"id":"review","executable":"/bin/echo","scope":"system","startup":"manual","restart":"never"}"#).unwrap();
    let spec = path.to_str().unwrap();
    for args in [
        vec!["plan", "install", "--spec", spec, "--dry-run"],
        vec![
            "install",
            "--spec",
            spec,
            "--backend",
            "systemd",
            "--priority",
            "bogus",
        ],
    ] {
        let (ok, value) = call(&args);
        assert!(!ok);
        assert_eq!(value["error"]["code"], "INVALID_ARGUMENTS");
    }
    let (ok, value) = call(&["plan", "uninstall", "--spec", spec, "--backend", "systemd"]);
    assert!(ok, "{value}");
    let actions = value["data"]["actions"].as_array().unwrap();
    assert_eq!(actions[0]["type"], "verifyDefinition");
    assert!(actions[0].get("expectedContent").is_some());
    let removal = actions.iter().find(|a| a["type"] == "removeFile").unwrap();
    assert_eq!(
        removal
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["expectedContent", "path", "type"]
    );
}
