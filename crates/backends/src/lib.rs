//! Built-in renderers, real manager probes and guarded supports_apply.
mod files;
#[cfg(windows)]
mod scm;
pub use files::{ensure_file, remove_file, verify_file};
use service_kit_core::{
    Backend, BackendCandidate, BackendRegistry, Capabilities, CommandCondition, Error, ErrorCode,
    HostContext, HostPlatform, Operation, PlanAction, Probe, RestartPolicy, Result, ScmStep, Scope,
    ServicePlan, ServiceSpec, ServiceState, ServiceStatus, Startup, is_windows_absolute_path,
};
use std::{
    path::PathBuf,
    process::{Command, Output},
};
#[derive(Clone, Copy, Debug)]
pub enum NativeBackend {
    Systemd,
    Launchd,
    Scm,
    OpenRc,
    Procd,
}
pub fn registry() -> BackendRegistry {
    let mut r = BackendRegistry::default();
    for b in [
        NativeBackend::Systemd,
        NativeBackend::Launchd,
        NativeBackend::Scm,
        NativeBackend::OpenRc,
        NativeBackend::Procd,
    ] {
        r.register(b).expect("unique built-ins");
    }
    r
}
pub fn host_context() -> HostContext {
    #[cfg(unix)]
    let uid = unsafe { libc::geteuid() };
    #[cfg(not(unix))]
    let uid = 0;
    HostContext {
        os: std::env::consts::OS.into(),
        home: std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .unwrap_or_default(),
        uid: if cfg!(unix) { Some(uid) } else { None },
        elevated: cfg!(unix) && uid == 0,
    }
}
fn run(program: &str, args: &[String]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .map_err(Into::into)
}
fn checked(program: &str, args: &[String]) -> Result<Output> {
    let out = run(program, args)?;
    if out.status.success() {
        Ok(out)
    } else {
        Err(Error::new(
            ErrorCode::CommandFailed,
            format!(
                "{program} {args:?}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ))
    }
}
fn strs(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}
fn unit(s: &ServiceSpec) -> String {
    format!("service-kit-{}.service", s.id.as_str())
}
fn label(s: &ServiceSpec) -> String {
    format!("service-kit.{}", s.id.as_str())
}
fn domain(s: &ServiceSpec, h: &HostContext) -> String {
    if s.scope == Scope::User {
        format!("gui/{}", h.uid.unwrap_or_default())
    } else {
        "system".into()
    }
}
fn systemctl(s: &ServiceSpec, command: &str) -> Vec<String> {
    let mut a = vec![];
    if s.scope == Scope::User {
        a.push("--user".into());
    }
    a.push(command.into());
    a
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn sd(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}
fn command(program: &str, args: Vec<String>) -> PlanAction {
    PlanAction::Command {
        executable: program.into(),
        args,
        condition: None,
        query_status: false,
    }
}
struct RenderedDefinition {
    path: PathBuf,
    content: String,
    make_executable: bool,
}
impl NativeBackend {
    fn os(self) -> HostPlatform {
        match self {
            Self::Launchd => HostPlatform::Macos,
            Self::Scm => HostPlatform::Windows,
            _ => HostPlatform::Linux,
        }
    }
    fn validate(self, s: &ServiceSpec) -> Result<()> {
        s.validate()?;
        if !self.capabilities().supports(s) {
            return Err(Error::new(
                ErrorCode::Unsupported,
                format!("{} does not support requested scope/restart", self.name()),
            ));
        }
        let o = &s.backend_options;
        if (o.systemd.is_some() && !matches!(self, Self::Systemd))
            || (o.launchd.is_some() && !matches!(self, Self::Launchd))
            || (o.scm.is_some() && !matches!(self, Self::Scm))
        {
            return Err(Error::new(
                ErrorCode::Unsupported,
                "backend options target a different backend",
            ));
        }
        let windows_path = is_windows_absolute_path(&s.executable);
        if windows_path != matches!(self, Self::Scm)
            || s.working_directory
                .as_ref()
                .is_some_and(|p| is_windows_absolute_path(p) != matches!(self, Self::Scm))
        {
            return Err(Error::new(
                ErrorCode::InvalidSpec,
                "path syntax does not match backend platform",
            ));
        }
        Ok(())
    }
    fn render(self, s: &ServiceSpec, h: &HostContext) -> Result<RenderedDefinition> {
        self.validate(s)?;
        let marker = format!("service-kit:v1:{}", s.id.as_str());
        let (path, content, make_executable) = match self {
            Self::Systemd => {
                let path = if s.scope == Scope::User {
                    h.home.join(".config/systemd/user")
                } else {
                    PathBuf::from("/etc/systemd/system")
                }
                .join(unit(s));
                let mut text = format!(
                    "# {marker}\n[Unit]\nDescription={}\n",
                    s.description.replace('%', "%%")
                );
                if s.backend_options
                    .systemd
                    .as_ref()
                    .is_some_and(|o| o.after_network)
                {
                    text.push_str("After=network-online.target\nWants=network-online.target\n");
                }
                text.push_str(&format!(
                    "\n[Service]\nType=simple\nExecStart={}\nRestart={}\n",
                    std::iter::once(&s.executable)
                        .chain(s.args.iter())
                        .map(|a| sd(a).replace('$', "$$"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    match s.restart {
                        RestartPolicy::Never => "no",
                        RestartPolicy::OnFailure => "on-failure",
                        RestartPolicy::Always => "always",
                    }
                ));
                if let Some(p) = &s.working_directory {
                    if p.trim() != p || p.ends_with('\\') {
                        return Err(Error::new(
                            ErrorCode::Unsupported,
                            "systemd WorkingDirectory cannot preserve edge whitespace or trailing backslash",
                        ));
                    }
                    text.push_str(&format!("WorkingDirectory={}\n", p.replace('%', "%%")));
                }
                for (k, v) in &s.environment {
                    text.push_str(&format!("Environment={}\n", sd(&format!("{k}={v}"))));
                }
                text.push_str("\n[Install]\nWantedBy=");
                text.push_str(if s.scope == Scope::User {
                    "default.target\n"
                } else {
                    "multi-user.target\n"
                });
                (path, text, false)
            }
            Self::Launchd => {
                let path = if s.scope == Scope::User {
                    h.home.join("Library/LaunchAgents")
                } else {
                    PathBuf::from("/Library/LaunchDaemons")
                }
                .join(format!("{}.plist", label(s)));
                let mut text = format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- {marker} -->\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{}</string>\n<key>ProgramArguments</key><array>\n",
                    label(s)
                );
                for a in std::iter::once(&s.executable).chain(s.args.iter()) {
                    text.push_str(&format!("<string>{}</string>\n", xml(a)));
                }
                text.push_str("</array>\n");
                if let Some(p) = &s.working_directory {
                    text.push_str(&format!(
                        "<key>WorkingDirectory</key><string>{}</string>\n",
                        xml(p)
                    ));
                }
                text.push_str("<key>EnvironmentVariables</key><dict>\n");
                for (k, v) in &s.environment {
                    text.push_str(&format!(
                        "<key>{}</key><string>{}</string>\n",
                        xml(k),
                        xml(v)
                    ));
                }
                text.push_str("</dict>\n");
                text.push_str(&format!(
                    "<key>RunAtLoad</key><{} />\n",
                    if s.startup == Startup::Automatic {
                        "true"
                    } else {
                        "false"
                    }
                ));
                text.push_str(match s.restart {
                    RestartPolicy::Never => "<key>KeepAlive</key><false/>\n",
                    RestartPolicy::Always => "<key>KeepAlive</key><true/>\n",
                    RestartPolicy::OnFailure => {
                        "<key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n"
                    }
                });
                // KeepAlive implies a first launch. Reject this semantic mismatch below for manual startup.
                if s.startup == Startup::Manual && s.restart != RestartPolicy::Never {
                    return Err(Error::new(
                        ErrorCode::Unsupported,
                        "launchd KeepAlive implies an initial launch; manual startup requires restart=never",
                    ));
                }
                if let Some(o) = &s.backend_options.launchd {
                    text.push_str(&format!(
                        "<key>ProcessType</key><string>{}</string>\n",
                        xml(&o.process_type)
                    ));
                }
                text.push_str("</dict></plist>\n");
                (path, text, false)
            }
            Self::OpenRc => {
                if !s.environment.is_empty() || s.working_directory.is_some() {
                    return Err(Error::new(
                        ErrorCode::Unsupported,
                        "OpenRC initial renderer does not support environment/cwd",
                    ));
                }
                let args = s.args.iter().map(|a| sq(a)).collect::<Vec<_>>().join(" ");
                (
                    PathBuf::from(format!("/etc/init.d/service-kit-{}", s.id.as_str())),
                    format!(
                        "#!/sbin/openrc-run\n# {marker}\ndescription={}\ncommand={}\ncommand_args={}\ncommand_background=true\npidfile=\"/run/service-kit-{}.pid\"\n",
                        sq(&s.description),
                        sq(&s.executable),
                        sq(&args),
                        s.id.as_str()
                    ),
                    true,
                )
            }
            Self::Procd => {
                let mut text = format!(
                    "#!/bin/sh /etc/rc.common\n# {marker}\nUSE_PROCD=1\nSTART=95\nSTOP=10\nstart_service() {{\n    procd_open_instance\n    procd_set_param command {}\n",
                    std::iter::once(&s.executable)
                        .chain(s.args.iter())
                        .map(|a| sq(a))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                for (k, v) in &s.environment {
                    text.push_str(&format!(
                        "    procd_append_param env {}\n",
                        sq(&format!("{k}={v}"))
                    ));
                }
                if let Some(p) = &s.working_directory {
                    text.push_str(&format!("    procd_set_param dir {}\n", sq(p)));
                }
                text.push_str("    procd_close_instance\n}\n");
                (
                    PathBuf::from(format!("/etc/init.d/service-kit-{}", s.id.as_str())),
                    text,
                    true,
                )
            }
            Self::Scm => (
                PathBuf::from(format!(
                    "C:\\ProgramData\\ServiceKit\\{}\\spec.json",
                    s.id.as_str()
                )),
                serde_json::to_string_pretty(s)
                    .map_err(|e| Error::new(ErrorCode::InvalidSpec, e.to_string()))?,
                false,
            ),
        };
        Ok(RenderedDefinition {
            path,
            content,
            make_executable,
        })
    }
}
impl Probe for NativeBackend {
    fn probe(&self, h: &HostContext, scope: Scope) -> BackendCandidate {
        let mut p = BackendCandidate {
            backend: self.name().into(),
            manager_available: false,
            reachable: false,
            can_write: false,
            diagnostics: vec![],
            capabilities: self.capabilities(),
        };
        if scope == Scope::User && h.elevated {
            p.diagnostics
                .push("Run user services from an unprivileged user session".into());
            return p;
        }
        if h.os != self.os() {
            p.diagnostics
                .push(format!("requires {}, host is {}", self.os(), h.os));
            return p;
        }
        if !p.capabilities.scopes.contains(&scope) {
            p.diagnostics.push("scope unsupported".into());
            return p;
        }
        let result = match self {
            Self::Systemd => {
                let mut args = if scope == Scope::User {
                    strs(&["--user"])
                } else {
                    vec![]
                };
                args.extend(strs(&["show", "--property=Version", "--value"]));
                run("/usr/bin/systemctl", &args)
            }
            Self::Launchd => run(
                "/bin/launchctl",
                &strs(&[
                    "print",
                    &if scope == Scope::User {
                        format!("gui/{}", h.uid.unwrap_or_default())
                    } else {
                        "system".into()
                    },
                ]),
            ),
            Self::Scm => {
                #[cfg(windows)]
                {
                    return scm::probe(p);
                }
                #[cfg(not(windows))]
                {
                    Err(Error::new(
                        ErrorCode::Unavailable,
                        "not compiled for Windows",
                    ))
                }
            }
            Self::OpenRc => run("/sbin/rc-status", &strs(&["--runlevel"])),
            Self::Procd => run("/sbin/ubus", &strs(&["call", "service", "list"])),
        };
        match result {
            Ok(out) => {
                p.manager_available = true;
                p.reachable = out.status.success();
                if !p.reachable {
                    p.diagnostics
                        .push(String::from_utf8_lossy(&out.stderr).trim().into());
                }
            }
            Err(e) => p.diagnostics.push(e.to_string()),
        }
        p.can_write = scope == Scope::User || h.elevated;
        if !p.can_write {
            p.diagnostics
                .push("system mutations require elevation".into());
        }
        if !p.capabilities.supports_apply {
            p.diagnostics.push("plan-only backend".into());
        }
        p
    }
}
impl Backend for NativeBackend {
    fn name(&self) -> &str {
        match self {
            Self::Systemd => "systemd",
            Self::Launchd => "launchd",
            Self::Scm => "scm",
            Self::OpenRc => "openrc",
            Self::Procd => "procd",
        }
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            scopes: match self {
                Self::Scm | Self::OpenRc | Self::Procd => vec![Scope::System],
                _ => vec![Scope::User, Scope::System],
            },
            restart_policies: match self {
                Self::OpenRc | Self::Procd => vec![RestartPolicy::Never],
                Self::Scm => vec![RestartPolicy::Never, RestartPolicy::OnFailure],
                _ => vec![
                    RestartPolicy::Never,
                    RestartPolicy::OnFailure,
                    RestartPolicy::Always,
                ],
            },
            supports_apply: !matches!(self, Self::OpenRc | Self::Procd),
            experimental: matches!(self, Self::Scm),
        }
    }
    fn plan(&self, s: &ServiceSpec, op: Operation, h: &HostContext) -> Result<ServicePlan> {
        if s.scope == Scope::User && h.elevated {
            return Err(Error::new(
                ErrorCode::InvalidHost,
                "Run user services from an unprivileged user session",
            ));
        }
        if s.scope == Scope::User && !h.home.is_absolute() {
            return Err(Error::new(
                ErrorCode::InvalidHost,
                "absolute HOME required for user service paths",
            ));
        }
        let RenderedDefinition {
            path,
            content,
            make_executable: executable,
        } = self.render(s, h)?;
        let mut actions = vec![];
        let mut warnings = vec![];
        if matches!(self, Self::OpenRc | Self::Procd) {
            if op != Operation::Install {
                return Err(Error::new(
                    ErrorCode::Unsupported,
                    "this backend only generates install plans",
                ));
            }
            warnings.push("config generation only; enable/start manually after review".into());
        }
        if matches!(self, Self::Scm) {
            warnings.push("experimental Windows host: requires elevated platform verification; child must tolerate forced termination".into());
            let directory =
                PathBuf::from(format!("C:\\ProgramData\\ServiceKit\\{}", s.id.as_str()));
            let host = PathBuf::from(format!("{}\\service-kit-host.exe", directory.display()));
            let spec_path = PathBuf::from(format!("{}\\spec.json", directory.display()));
            let mut steps = vec![ScmStep::VerifyOwnership {
                service_name: unit(s).trim_end_matches(".service").into(),
                directory: directory.clone(),
                absent_is_success: matches!(op, Operation::Uninstall | Operation::Status),
            }];
            match op {
                Operation::Install => {
                    for path in [
                        PathBuf::from(r"C:\ProgramData\ServiceKit"),
                        directory.clone(),
                    ] {
                        steps.push(ScmStep::EnsureProtectedDirectory {
                            path,
                            sddl: "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)".into(),
                        });
                    }
                    steps.extend([
                        ScmStep::WriteSpec {
                            path: spec_path.clone(),
                            content,
                        },
                        ScmStep::CopyHost {
                            source: std::env::current_exe()?.with_file_name("service-kit-host.exe"),
                            destination: host.clone(),
                        },
                        ScmStep::CreateService {
                            service_name: format!("service-kit-{}", s.id.as_str()),
                            host,
                            spec_path,
                            startup: s.startup,
                            description: s.description.clone(),
                        },
                        ScmStep::ConfigureRecovery {
                            restart: s.restart,
                            reset_secs: 86400,
                            delay_secs: 5,
                        },
                    ]);
                }
                Operation::Stop | Operation::Restart | Operation::Uninstall => {
                    steps.push(ScmStep::StopIfRunning {
                        timeout_secs: u64::from(
                            s.backend_options
                                .scm
                                .as_ref()
                                .map_or(20, |o| o.stop_timeout_secs),
                        ) + 10,
                    })
                }
                _ => {}
            }
            if matches!(op, Operation::Start | Operation::Restart) {
                steps.push(ScmStep::StartIfStopped { timeout_secs: 30 });
            }
            if op == Operation::Uninstall {
                steps.push(ScmStep::DeleteService);
                steps.push(ScmStep::RemoveHostFiles {
                    directory,
                    files: [
                        "spec.json",
                        "service-kit-host.exe",
                        "stdout.log",
                        "stderr.log",
                        "host-errors.log",
                    ]
                    .map(str::to_owned)
                    .into(),
                });
            } else {
                steps.push(ScmStep::QueryStatus);
            }
            actions.extend(steps.into_iter().map(|step| {
                let only_if_absent = matches!(
                    step,
                    ScmStep::EnsureProtectedDirectory { .. }
                        | ScmStep::WriteSpec { .. }
                        | ScmStep::CopyHost { .. }
                        | ScmStep::CreateService { .. }
                        | ScmStep::ConfigureRecovery { .. }
                );
                PlanAction::Scm {
                    step,
                    only_if_absent,
                }
            }));
        } else {
            if op != Operation::Install {
                actions.push(PlanAction::VerifyDefinition {
                    path: path.clone(),
                    expected_content: content.clone(),
                    absent_is_success: matches!(op, Operation::Status | Operation::Uninstall),
                    verify_content: op != Operation::Status,
                });
            }
            if op == Operation::Install {
                actions.push(PlanAction::WriteFile {
                    path: path.clone(),
                    content: content.clone(),
                    make_executable: executable,
                });
            }
            match self {
                Self::Systemd => {
                    let mut add = |verb: &str, with_unit: bool| {
                        let mut args = systemctl(s, verb);
                        if with_unit {
                            args.push(unit(s));
                        }
                        actions.push(command("/usr/bin/systemctl", args));
                    };
                    match op {
                        Operation::Install => {
                            add("daemon-reload", false);
                            if s.startup == Startup::Automatic {
                                add("enable", true);
                            }
                        }
                        Operation::Uninstall => {
                            add("stop", true);
                            add("disable", true);
                        }
                        Operation::Start => add("start", true),
                        Operation::Stop => add("stop", true),
                        Operation::Restart => add("restart", true),
                        Operation::Status => {
                            let mut args = systemctl(s, "show");
                            args.extend([
                                unit(s),
                                "--property=ActiveState".into(),
                                "--value".into(),
                            ]);
                            actions.push(command("/usr/bin/systemctl", args));
                        }
                    }
                }
                Self::Launchd => {
                    let target = format!("{}/{}", domain(s, h), label(s));
                    match op {
                        Operation::Install => actions.push(command(
                            "/bin/launchctl",
                            vec![
                                "bootstrap".into(),
                                domain(s, h),
                                path.to_string_lossy().into(),
                            ],
                        )),
                        Operation::Uninstall => {
                            actions.push(command("/bin/launchctl", vec!["bootout".into(), target]))
                        }
                        Operation::Start => actions
                            .push(command("/bin/launchctl", vec!["kickstart".into(), target])),
                        Operation::Restart => actions.push(command(
                            "/bin/launchctl",
                            vec!["kickstart".into(), "-k".into(), target],
                        )),
                        Operation::Stop => {
                            if s.restart != RestartPolicy::Never {
                                return Err(Error::new(
                                    ErrorCode::Unsupported,
                                    "launchd KeepAlive would restart after stop; uninstall or use restart=never",
                                ));
                            }
                            actions.push(command(
                                "/bin/launchctl",
                                vec!["kill".into(), "SIGTERM".into(), target],
                            ));
                        }
                        Operation::Status => {
                            actions.push(command("/bin/launchctl", vec!["print".into(), target]))
                        }
                    }
                }
                _ => {}
            }
            if matches!(self, Self::Launchd)
                && matches!(op, Operation::Install | Operation::Uninstall)
            {
                for action in &mut actions {
                    if let PlanAction::Command { condition, .. } = action {
                        let target = format!("{}/{}", domain(s, h), label(s));
                        *condition = Some(if op == Operation::Install {
                            CommandCondition::LaunchdJobAbsent {
                                domain: domain(s, h),
                                target,
                                expected_path: path.clone(),
                            }
                        } else {
                            CommandCondition::LaunchdJobPresent {
                                domain: domain(s, h),
                                target,
                                expected_path: path.clone(),
                            }
                        });
                    }
                }
            }
            if op == Operation::Uninstall {
                actions.push(PlanAction::RemoveFile {
                    path,
                    expected_content: content,
                });
                if matches!(self, Self::Systemd) {
                    actions.push(command("/usr/bin/systemctl", systemctl(s, "daemon-reload")));
                }
            }
        }
        if !matches!(self, Self::Scm | Self::OpenRc | Self::Procd) && op != Operation::Uninstall {
            if op != Operation::Status {
                let (executable, args) = self.status_command(s, h);
                actions.push(command(executable, args));
            }
            if let Some(PlanAction::Command { query_status, .. }) = actions.last_mut() {
                *query_status = true;
            }
        }
        Ok(ServicePlan {
            backend: self.name().into(),
            operation: op,
            service_id: s.id.clone(),
            scope: s.scope,
            actions,
            warnings,
        })
    }
    fn apply(&self, s: &ServiceSpec, op: Operation, h: &HostContext) -> Result<ServiceStatus> {
        // Never execute deserialized actions: regenerate from validated spec and trusted context.
        let plan = self.plan(s, op, h)?;
        let p = self.probe(h, s.scope);
        if !p.usable_for(op) {
            return Err(Error::new(
                if op != Operation::Status && !p.can_write {
                    ErrorCode::PermissionDenied
                } else {
                    ErrorCode::Unavailable
                },
                format!("{}: {}", self.name(), p.diagnostics.join("; ")),
            ));
        }
        #[cfg(windows)]
        if matches!(self, Self::Scm) {
            return scm::apply(s, op, &plan.actions);
        }
        for action in plan.actions {
            match action {
                PlanAction::WriteFile {
                    path,
                    content,
                    make_executable: executable,
                } => {
                    ensure_file(&path, &content, executable)?;
                }
                PlanAction::VerifyDefinition {
                    path,
                    expected_content,
                    absent_is_success,
                    verify_content,
                } => {
                    if !path.try_exists()? {
                        if absent_is_success {
                            return Ok(ServiceStatus {
                                state: ServiceState::NotInstalled,
                                detail: "managed definition absent".into(),
                            });
                        }
                        return Err(Error::new(ErrorCode::NotFound, "managed definition absent"));
                    }
                    if verify_content {
                        verify_file(&path, &expected_content)?;
                    }
                }
                PlanAction::RemoveFile {
                    path,
                    expected_content,
                } => remove_file(&path, &expected_content)?,
                PlanAction::Command {
                    executable,
                    args,
                    condition,
                    query_status,
                } => {
                    if let Some(condition) = condition
                        && !launchd_condition(&condition)?
                    {
                        continue;
                    }
                    let output = checked(&executable, &args)?;
                    if query_status {
                        return Ok(self.parse_status(output));
                    }
                }
                PlanAction::Scm { .. } => {
                    return Err(Error::new(
                        ErrorCode::Unsupported,
                        "SCM not compiled on this host",
                    ));
                }
            }
        }
        if op == Operation::Uninstall {
            Ok(ServiceStatus {
                state: ServiceState::NotInstalled,
                detail: "managed definition removed".into(),
            })
        } else {
            Ok(ServiceStatus {
                state: ServiceState::Unknown,
                detail: "plan applied".into(),
            })
        }
    }
}
impl NativeBackend {
    fn status_command(self, s: &ServiceSpec, h: &HostContext) -> (&'static str, Vec<String>) {
        match self {
            Self::Systemd => {
                let mut args = systemctl(s, "show");
                args.extend([unit(s), "--property=ActiveState".into(), "--value".into()]);
                ("/usr/bin/systemctl", args)
            }
            _ => (
                "/bin/launchctl",
                vec!["print".into(), format!("{}/{}", domain(s, h), label(s))],
            ),
        }
    }
    fn parse_status(self, output: Output) -> ServiceStatus {
        let detail = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let state = match self {
            Self::Systemd => match detail.as_str() {
                "active" | "activating" => ServiceState::Running,
                "inactive" | "deactivating" => ServiceState::Stopped,
                "failed" => ServiceState::Failed,
                _ => ServiceState::Unknown,
            },
            _ if detail.lines().any(|line| line.trim() == "state = running") => {
                ServiceState::Running
            }
            _ => ServiceState::Stopped,
        };
        ServiceStatus { state, detail }
    }
}
fn launchd_condition(condition: &CommandCondition) -> Result<bool> {
    launchd_condition_with(condition, |args| run("/bin/launchctl", args))
}
fn launchd_condition_with(
    condition: &CommandCondition,
    mut query: impl FnMut(&[String]) -> Result<Output>,
) -> Result<bool> {
    let (domain, target, expected_path, want_present) = match condition {
        CommandCondition::LaunchdJobAbsent {
            domain,
            target,
            expected_path,
        } => (domain, target, expected_path, false),
        CommandCondition::LaunchdJobPresent {
            domain,
            target,
            expected_path,
        } => (domain, target, expected_path, true),
    };
    let output = query(&["print".into(), target.clone()])?;
    if output.status.success() {
        verify_launchd_path(&String::from_utf8_lossy(&output.stdout), expected_path)?;
        return Ok(want_present);
    }
    // Only a missing job in a reachable domain is idempotent; propagate permission/manager failures.
    let domain_output = query(&["print".into(), domain.clone()])?;
    if !domain_output.status.success() {
        return Err(Error::new(
            ErrorCode::Unavailable,
            String::from_utf8_lossy(&domain_output.stderr),
        ));
    }
    if String::from_utf8_lossy(&output.stderr).contains("Could not find service") {
        return Ok(!want_present);
    }
    Err(Error::new(
        ErrorCode::CommandFailed,
        String::from_utf8_lossy(&output.stderr),
    ))
}
fn verify_launchd_path(output: &str, expected: &std::path::Path) -> Result<()> {
    let actual = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("path = "));
    if actual != expected.to_str() {
        return Err(Error::new(
            ErrorCode::Conflict,
            "launchd label belongs to a different definition",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests;
