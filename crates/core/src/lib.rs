//! Versioned domain model. No manager detection or side effects live here.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const PROTOCOL_VERSION: u32 = 1;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidSpec,
    Unsupported,
    Unavailable,
    PermissionDenied,
    Conflict,
    Io,
    CommandFailed,
    NotFound,
    InvalidArguments,
    InvalidHost,
    UnsafePath,
    ManagerFailed,
    Timeout,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}
impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            serde_json::to_value(&self.code)
                .map_err(|_| std::fmt::Error)?
                .as_str()
                .unwrap_or("UNKNOWN"),
            self.message
        )
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                ErrorCode::PermissionDenied
            } else {
                ErrorCode::Io
            },
            e.to_string(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct ServiceId(String);
impl ServiceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for ServiceId {
    type Error = Error;
    fn try_from(s: String) -> Result<Self> {
        if s.is_empty()
            || s.len() > 128
            || !s.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::new(
                ErrorCode::InvalidSpec,
                "id must be 1..128 ASCII letters/digits/._-, start alphanumeric",
            ));
        }
        Ok(Self(s))
    }
}
impl From<ServiceId> for String {
    fn from(id: ServiceId) -> Self {
        id.0
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    User,
    System,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Startup {
    Manual,
    Automatic,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    Install,
    Uninstall,
    Start,
    Stop,
    Restart,
    Status,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendOptions {
    pub systemd: Option<SystemdOptions>,
    pub launchd: Option<LaunchdOptions>,
    pub scm: Option<ScmOptions>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemdOptions {
    pub after_network: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchdOptions {
    pub process_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScmOptions {
    pub stop_timeout_secs: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceSpec {
    pub id: ServiceId,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub description: String,
    pub scope: Scope,
    pub startup: Startup,
    pub restart: RestartPolicy,
    #[serde(default)]
    pub backend_options: BackendOptions,
}
fn is_safe_text(s: &str) -> bool {
    !s.chars().any(|c| c.is_control())
}
pub fn is_absolute_path(s: &str) -> bool {
    s.starts_with('/') || is_windows_absolute_path(s)
}
pub fn is_windows_absolute_path(s: &str) -> bool {
    s.len() > 3
        && s.as_bytes()[0].is_ascii_alphabetic()
        && s.as_bytes()[1] == b':'
        && matches!(s.as_bytes()[2], b'\\' | b'/')
}
fn is_safe_path(s: &str) -> bool {
    is_absolute_path(s) && is_safe_text(s) && !s.split(['/', '\\']).any(|p| p == "..")
}
impl ServiceSpec {
    pub fn validate(&self) -> Result<()> {
        // Windows environment is case insensitive. Reserve the host's stop contract.
        let mut keys = std::collections::BTreeSet::new();
        for key in self.environment.keys() {
            let normalized = key.to_uppercase();
            if normalized == "SERVICE_KIT_STOP_EVENT"
                || (is_windows_absolute_path(&self.executable) && !keys.insert(normalized))
            {
                return Err(Error::new(
                    ErrorCode::InvalidSpec,
                    "reserved or duplicate Windows environment variable",
                ));
            }
        }

        if !is_safe_path(&self.executable)
            || self
                .working_directory
                .as_ref()
                .is_some_and(|p| !is_safe_path(p))
        {
            return Err(Error::new(
                ErrorCode::InvalidSpec,
                "executable and workingDirectory must be absolute paths without dot components or control characters",
            ));
        }
        if !is_safe_text(&self.description)
            || self.args.iter().any(|a| !is_safe_text(a))
            || self.environment.iter().any(|(k, v)| {
                k.is_empty()
                    || !k.bytes().enumerate().all(|(i, b)| {
                        b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                    })
                    || !is_safe_text(v)
            })
        {
            return Err(Error::new(
                ErrorCode::InvalidSpec,
                "invalid environment name or control characters in service values",
            ));
        }
        if self.backend_options.launchd.as_ref().is_some_and(|o| {
            !["Background", "Standard", "Interactive", "Adaptive"]
                .contains(&o.process_type.as_str())
        }) || self
            .backend_options
            .scm
            .as_ref()
            .is_some_and(|o| o.stop_timeout_secs == 0 || o.stop_timeout_secs > 300)
        {
            return Err(Error::new(ErrorCode::InvalidSpec, "invalid backend option"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HostPlatform {
    Linux,
    Macos,
    Windows,
    Other,
}
impl From<&str> for HostPlatform {
    fn from(value: &str) -> Self {
        match value {
            "linux" => Self::Linux,
            "macos" => Self::Macos,
            "windows" => Self::Windows,
            _ => Self::Other,
        }
    }
}
impl std::fmt::Display for HostPlatform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Other => "other",
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostContext {
    pub os: HostPlatform,
    pub home: PathBuf,
    pub uid: Option<u32>,
    pub elevated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub scopes: Vec<Scope>,
    pub restart_policies: Vec<RestartPolicy>,
    pub supports_apply: bool,
    pub experimental: bool,
}
impl Capabilities {
    pub fn supports(&self, s: &ServiceSpec) -> bool {
        self.scopes.contains(&s.scope) && self.restart_policies.contains(&s.restart)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCandidate {
    pub backend: String,
    pub manager_available: bool,
    pub reachable: bool,
    pub can_write: bool,
    pub diagnostics: Vec<String>,
    pub capabilities: Capabilities,
}
impl BackendCandidate {
    pub fn usable_for(&self, operation: Operation) -> bool {
        self.manager_available
            && self.reachable
            && (operation == Operation::Status || self.can_write)
            && self.capabilities.supports_apply
    }
    pub fn exclusion(&self, spec: &ServiceSpec, operation: Operation) -> Option<String> {
        let mut reasons = self.diagnostics.clone();
        if !self.capabilities.supports(spec) {
            reasons.push("scope or restart policy unsupported".into());
        }
        if self.usable_for(operation) && self.capabilities.supports(spec) {
            return None;
        }
        if !self.manager_available {
            reasons.push("manager unavailable".into());
        }
        if !self.reachable {
            reasons.push("manager unreachable".into());
        }
        if operation != Operation::Status && !self.can_write {
            reasons.push("write permission required".into());
        }
        if !self.capabilities.supports_apply {
            reasons.push("plan-only backend".into());
        }
        Some(format!("{}: {}", self.backend, reasons.join("; ")))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CommandCondition {
    LaunchdJobAbsent {
        domain: String,
        target: String,
        expected_path: PathBuf,
    },
    LaunchdJobPresent {
        domain: String,
        target: String,
        expected_path: PathBuf,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlanAction {
    WriteFile {
        path: PathBuf,
        content: String,
        make_executable: bool,
    },
    VerifyDefinition {
        path: PathBuf,
        expected_content: String,
        absent_is_success: bool,
        verify_content: bool,
    },
    RemoveFile {
        path: PathBuf,
        expected_content: String,
    },
    Command {
        executable: String,
        args: Vec<String>,
        condition: Option<CommandCondition>,
        query_status: bool,
    },
    Scm {
        step: ScmStep,
        only_if_absent: bool,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScmStep {
    VerifyOwnership {
        service_name: String,
        directory: PathBuf,
        absent_is_success: bool,
    },
    EnsureProtectedDirectory {
        path: PathBuf,
        sddl: String,
    },
    WriteSpec {
        path: PathBuf,
        content: String,
    },
    CopyHost {
        source: PathBuf,
        destination: PathBuf,
    },
    CreateService {
        service_name: String,
        host: PathBuf,
        spec_path: PathBuf,
        startup: Startup,
        description: String,
    },
    ConfigureRecovery {
        restart: RestartPolicy,
        reset_secs: u64,
        delay_secs: u64,
    },
    StopIfRunning {
        timeout_secs: u64,
    },
    StartIfStopped {
        timeout_secs: u64,
    },
    DeleteService,
    RemoveHostFiles {
        directory: PathBuf,
        files: Vec<String>,
    },
    QueryStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePlan {
    pub backend: String,
    pub operation: Operation,
    pub service_id: ServiceId,
    pub scope: Scope,
    pub actions: Vec<PlanAction>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ServiceState {
    Running,
    Stopped,
    Failed,
    NotInstalled,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub state: ServiceState,
    pub detail: String,
}
pub trait Probe {
    fn probe(&self, host: &HostContext, scope: Scope) -> BackendCandidate;
}
pub trait Backend: Probe + Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    fn plan(
        &self,
        spec: &ServiceSpec,
        operation: Operation,
        host: &HostContext,
    ) -> Result<ServicePlan>;
    fn apply(
        &self,
        spec: &ServiceSpec,
        operation: Operation,
        host: &HostContext,
    ) -> Result<ServiceStatus>;
}
pub trait Strategy {
    fn select(
        &self,
        candidates: &[BackendCandidate],
        spec: &ServiceSpec,
        operation: Operation,
    ) -> Result<String>;
}
pub struct FixedBackendStrategy(pub String);
impl Strategy for FixedBackendStrategy {
    fn select(
        &self,
        candidates: &[BackendCandidate],
        spec: &ServiceSpec,
        operation: Operation,
    ) -> Result<String> {
        let candidate = candidates
            .iter()
            .find(|p| p.backend == self.0)
            .ok_or_else(|| {
                Error::new(ErrorCode::NotFound, format!("unknown backend: {}", self.0))
            })?;
        match candidate.exclusion(spec, operation) {
            None => Ok(candidate.backend.clone()),
            Some(reason) => Err(Error::new(ErrorCode::Unavailable, reason)),
        }
    }
}
pub struct PriorityStrategy {
    pub priorities: Vec<String>,
}
impl Default for PriorityStrategy {
    fn default() -> Self {
        Self {
            priorities: ["systemd", "launchd", "scm", "openrc", "procd"]
                .map(str::to_owned)
                .into(),
        }
    }
}
impl Strategy for PriorityStrategy {
    fn select(
        &self,
        candidates: &[BackendCandidate],
        spec: &ServiceSpec,
        operation: Operation,
    ) -> Result<String> {
        let mut exclusions = Vec::new();
        for name in &self.priorities {
            match FixedBackendStrategy(name.clone()).select(candidates, spec, operation) {
                Ok(name) => return Ok(name),
                Err(error) => exclusions.push(error.message),
            }
        }
        Err(Error::new(
            ErrorCode::Unavailable,
            format!("No eligible backend: {}", exclusions.join(" | ")),
        ))
    }
}
#[derive(Default)]
pub struct BackendRegistry {
    backends: Vec<Box<dyn Backend>>,
}
impl BackendRegistry {
    pub fn register(&mut self, b: impl Backend + 'static) -> Result<()> {
        if self.backends.iter().any(|x| x.name() == b.name()) {
            return Err(Error::new(ErrorCode::Conflict, "duplicate backend"));
        }
        self.backends.push(Box::new(b));
        Ok(())
    }
    pub fn get(&self, name: &str) -> Result<&dyn Backend> {
        self.backends
            .iter()
            .find(|b| b.name() == name)
            .map(|b| b.as_ref())
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("unknown backend: {name}")))
    }
    pub fn probes(&self, h: &HostContext, scope: Scope) -> Vec<BackendCandidate> {
        self.backends.iter().map(|b| b.probe(h, scope)).collect()
    }
    pub fn select(
        &self,
        strategy: &dyn Strategy,
        operation: Operation,
        s: &ServiceSpec,
        h: &HostContext,
    ) -> Result<&dyn Backend> {
        self.get(&strategy.select(&self.probes(h, s.scope), s, operation)?)
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T: Serialize> {
    pub protocol_version: u32,
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<Error>,
}
impl<T: Serialize> Envelope<T> {
    pub fn from_result(r: Result<T>) -> Self {
        match r {
            Ok(data) => Self {
                protocol_version: PROTOCOL_VERSION,
                success: true,
                data: Some(data),
                error: None,
            },
            Err(error) => Self {
                protocol_version: PROTOCOL_VERSION,
                success: false,
                data: None,
                error: Some(error),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn spec() -> ServiceSpec {
        serde_json::from_value(serde_json::json!({"id":"demo", "executable":"/bin/demo", "scope":"user", "startup":"manual", "restart":"never"})).unwrap()
    }
    #[test]
    fn validation() {
        for id in ["", "../x", "-x", "a/b", "a\n"] {
            assert!(ServiceId::try_from(id.to_string()).is_err());
        }
        let mut s = spec();
        s.executable = "/bin/../x".into();
        assert!(s.validate().is_err());
        s.executable = "C:\\Apps\\demo.exe".into();
        assert!(s.validate().is_ok());
        s.environment.insert("A=B".into(), "x".into());
        assert!(s.validate().is_err());
    }
    #[test]
    fn capability_selection() {
        let mut s = spec();
        let caps = Capabilities {
            scopes: vec![Scope::User],
            restart_policies: vec![RestartPolicy::Never],
            supports_apply: true,
            experimental: false,
        };
        let p = BackendCandidate {
            backend: "custom".into(),
            manager_available: true,
            reachable: true,
            can_write: true,
            diagnostics: vec![],
            capabilities: caps,
        };
        let a = PriorityStrategy {
            priorities: vec!["missing".into(), "custom".into()],
        };
        assert_eq!(
            a.select(std::slice::from_ref(&p), &s, Operation::Install)
                .unwrap(),
            "custom"
        );
        s.restart = RestartPolicy::Always;
        assert!(
            a.select(std::slice::from_ref(&p), &s, Operation::Install)
                .is_err()
        );
        s.restart = RestartPolicy::Never;
        let mut unreachable = p;
        unreachable.reachable = false;
        assert!(a.select(&[unreachable], &s, Operation::Install).is_err());
    }
}

#[cfg(test)]
mod operation_selection_tests {
    use super::*;
    #[test]
    fn status_accepts_read_only_candidates_and_reports_mutation_exclusions() {
        let spec = tests::spec();
        let candidate = BackendCandidate {
            backend: "systemd".into(),
            manager_available: true,
            reachable: true,
            can_write: false,
            diagnostics: vec![],
            capabilities: Capabilities {
                scopes: vec![Scope::User],
                restart_policies: vec![RestartPolicy::Never],
                supports_apply: true,
                experimental: false,
            },
        };
        let strategy = PriorityStrategy {
            priorities: vec!["missing".into(), "systemd".into()],
        };
        assert_eq!(
            strategy
                .select(std::slice::from_ref(&candidate), &spec, Operation::Status)
                .unwrap(),
            "systemd"
        );
        let error = strategy
            .select(&[candidate], &spec, Operation::Install)
            .unwrap_err();
        assert!(error.message.contains("unknown backend: missing"));
        assert!(error.message.contains("systemd: write permission required"));
    }
}
