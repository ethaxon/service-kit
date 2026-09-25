use super::*;
fn spec() -> ServiceSpec {
    serde_json::from_value(serde_json::json!({"id":"demo", "executable":"/bin/echo", "args":["a b", "a\"b", "$HOME;%n&<>"], "environment":{"VALUE":"%n $HOME & <x>"}, "scope":"user", "startup":"manual", "restart":"never"})).unwrap()
}
fn host() -> HostContext {
    HostContext {
        os: "macos".into(),
        home: PathBuf::from("/Users/test"),
        uid: Some(501),
        elevated: false,
    }
}
fn content(p: &ServicePlan) -> &str {
    match &p.actions[0] {
        PlanAction::WriteFile { content, .. } => content,
        _ => panic!("not a write"),
    }
}
#[test]
fn systemd_escaping() {
    let p = NativeBackend::Systemd
        .plan(&spec(), Operation::Install, &host())
        .unwrap();
    let c = content(&p);
    assert!(c.contains("\"$$HOME;%%n&<>\""));
    assert!(c.contains("\"a\\\"b\""));
    assert!(c.contains("Environment=\"VALUE=%%n $HOME & <x>\""));
}
#[test]
fn launchd_xml_and_paths() {
    let p = NativeBackend::Launchd
        .plan(&spec(), Operation::Install, &host())
        .unwrap();
    assert!(content(&p).contains("$HOME;%n&amp;&lt;&gt;"));
    assert!(content(&p).contains("a&quot;b"));
    assert!(content(&p).contains("<key>KeepAlive</key><false/>"));
    match &p.actions[0] {
        PlanAction::WriteFile { path, .. } => assert_eq!(
            path,
            &PathBuf::from("/Users/test/Library/LaunchAgents/service-kit.demo.plist")
        ),
        _ => panic!(),
    }
}
#[test]
fn semantics_rejected() {
    let mut s = spec();
    s.restart = RestartPolicy::OnFailure;
    assert!(
        NativeBackend::Launchd
            .plan(&s, Operation::Install, &host())
            .is_err()
    );
    s.scope = Scope::System;
    assert!(
        NativeBackend::OpenRc
            .plan(&s, Operation::Install, &host())
            .is_err()
    );
    s.restart = RestartPolicy::Never;
    assert!(
        NativeBackend::Scm
            .plan(&s, Operation::Install, &host())
            .is_err()
    );
    s.scope = Scope::User;
    s.executable = "C:\\Apps\\demo.exe".into();
    assert!(
        NativeBackend::Scm
            .plan(&s, Operation::Install, &host())
            .is_err()
    );
}
#[test]
fn openrc_procd_escape_arguments_and_reject_execution() {
    let mut s = spec();
    s.scope = Scope::System;
    s.environment.clear();
    s.args = vec!["a'b;$(x)".into()];
    for b in [NativeBackend::OpenRc, NativeBackend::Procd] {
        let p = b.plan(&s, Operation::Install, &host()).unwrap();
        assert!(!b.capabilities().supports_apply);
        assert!(!p.warnings.is_empty());
        assert!(content(&p).contains("'\\''"));
        assert!(b.plan(&s, Operation::Start, &host()).is_err());
    }
}
#[test]
fn foreign_probe_explains_exclusion() {
    let p = NativeBackend::Systemd.probe(&host(), Scope::User);
    assert!(!p.usable_for(Operation::Install));
    assert!(p.diagnostics[0].contains("requires linux"));
}
#[test]
fn file_install_is_atomic_idempotent_and_protects_unrelated() {
    let t = test_directory();
    let root = t.path().canonicalize().unwrap();
    let p = root.join("nested/service");
    assert!(ensure_file(&p, "owned", false).unwrap());
    assert!(!ensure_file(&p, "owned", false).unwrap());
    assert!(ensure_file(&p, "different", false).is_err());
    assert!(remove_file(&p, "different").is_err());
    remove_file(&p, "owned").unwrap();
    remove_file(&p, "owned").unwrap();
    assert!(!p.exists());
}
#[cfg(unix)]
#[test]
fn refuses_symlink_paths() {
    use std::os::unix::fs::symlink;
    let t = test_directory();
    let root = t.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("real")).unwrap();
    symlink(root.join("real"), root.join("link")).unwrap();
    assert!(ensure_file(&root.join("link/file"), "x", false).is_err());
    assert!(!root.join("real/file").exists());
}

fn test_directory() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../temp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

#[test]
fn plans_expose_status_commands_and_launchd_conditions() {
    let plan = NativeBackend::Systemd
        .plan(&spec(), Operation::Status, &host())
        .unwrap();
    match &plan.actions[1] {
        PlanAction::Command {
            args, query_status, ..
        } => {
            assert_eq!(
                args,
                &[
                    "--user",
                    "show",
                    "service-kit-demo.service",
                    "--property=ActiveState",
                    "--value"
                ]
            );
            assert!(query_status);
        }
        _ => panic!("expected status command"),
    }
    let plan = NativeBackend::Launchd
        .plan(&spec(), Operation::Uninstall, &host())
        .unwrap();
    assert!(matches!(
        plan.actions[1],
        PlanAction::Command {
            condition: Some(CommandCondition::LaunchdJobPresent { .. }),
            ..
        }
    ));
    assert!(matches!(plan.actions[2], PlanAction::RemoveFile { .. }));
    assert!(
        verify_launchd_path(
            "path = /some/other.plist",
            std::path::Path::new("/expected.plist")
        )
        .is_err()
    );
}

#[test]
fn elevated_user_scope_is_rejected() {
    let mut context = host();
    context.elevated = true;
    context.uid = Some(0);
    assert!(
        NativeBackend::Launchd
            .plan(&spec(), Operation::Install, &context)
            .is_err()
    );
    assert!(
        !NativeBackend::Launchd
            .probe(&context, Scope::User)
            .usable_for(Operation::Status)
    );
}

#[test]
fn working_directory_is_an_unquoted_systemd_path() {
    let mut spec = spec();
    spec.working_directory = Some("/opt/my app/%data".into());
    let plan = NativeBackend::Systemd
        .plan(&spec, Operation::Install, &host())
        .unwrap();
    assert!(content(&plan).contains("WorkingDirectory=/opt/my app/%%data\n"));
    #[cfg(target_os = "linux")]
    {
        let temp = test_directory();
        let path = temp.path().join("service-kit-review.service");
        std::fs::write(&path, content(&plan)).unwrap();
        let output = std::process::Command::new("systemd-analyze")
            .arg("verify")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("WorkingDirectory"));
    }
}

#[cfg(unix)]
#[test]
fn unloaded_launchd_job_skips_bootout_but_manager_failures_propagate() {
    use std::os::unix::process::ExitStatusExt;
    let condition = CommandCondition::LaunchdJobPresent {
        domain: "gui/501".into(),
        target: "gui/501/service-kit.demo".into(),
        expected_path: "/tmp/demo.plist".into(),
    };
    let output = |code, stderr: &str| Output {
        status: std::process::ExitStatus::from_raw(code << 8),
        stdout: vec![],
        stderr: stderr.as_bytes().to_vec(),
    };
    let mut calls = 0;
    let should_run = launchd_condition_with(&condition, |_| {
        calls += 1;
        Ok(if calls == 1 {
            output(113, "Could not find service")
        } else {
            output(0, "")
        })
    })
    .unwrap();
    assert!(!should_run);
    assert_eq!(calls, 2);
    assert!(launchd_condition_with(&condition, |_| Ok(output(1, "permission denied"))).is_err());
}

#[test]
fn scm_plan_exposes_installation_steps_and_conditions() {
    let mut spec = spec();
    spec.scope = Scope::System;
    spec.executable = r"C:\Apps\worker.exe".into();
    let plan = NativeBackend::Scm
        .plan(&spec, Operation::Install, &host())
        .unwrap();
    let steps: Vec<_> = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            PlanAction::Scm {
                step,
                only_if_absent,
            } => Some((step, only_if_absent)),
            _ => None,
        })
        .collect();
    assert!(matches!(steps[0], (ScmStep::VerifyOwnership { .. }, false)));
    assert!(steps.iter().any(|(step, conditional)| matches!(step, ScmStep::EnsureProtectedDirectory { sddl, .. } if sddl.contains(";;;BA")) && **conditional));
    assert!(steps.iter().any(|(step, conditional)| matches!(
        step,
        ScmStep::ConfigureRecovery {
            reset_secs: 86400,
            delay_secs: 5,
            ..
        }
    ) && **conditional));
    assert!(matches!(
        steps.last().unwrap(),
        (ScmStep::QueryStatus, false)
    ));
}
