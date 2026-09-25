export const HostPlatform = {
  Linux: "linux",
  Macos: "macos",
  Windows: "windows",
  Other: "other",
} as const;
export type HostPlatform = (typeof HostPlatform)[keyof typeof HostPlatform];

export const ServiceState = {
  Running: "running",
  Stopped: "stopped",
  Failed: "failed",
  NotInstalled: "notInstalled",
  Unknown: "unknown",
} as const;
export type ServiceState = (typeof ServiceState)[keyof typeof ServiceState];

export const Startup = {
  Manual: "manual",
  Automatic: "automatic",
} as const;
export type Startup = (typeof Startup)[keyof typeof Startup];

export const CommandConditionType = {
  LaunchdJobAbsent: "launchdJobAbsent",
  LaunchdJobPresent: "launchdJobPresent",
} as const;
export type CommandConditionType = (typeof CommandConditionType)[keyof typeof CommandConditionType];

export const ScmStepType = {
  VerifyOwnership: "verifyOwnership",
  EnsureProtectedDirectory: "ensureProtectedDirectory",
  WriteSpec: "writeSpec",
  CopyHost: "copyHost",
  CreateService: "createService",
  ConfigureRecovery: "configureRecovery",
  StopIfRunning: "stopIfRunning",
  StartIfStopped: "startIfStopped",
  DeleteService: "deleteService",
  QueryStatus: "queryStatus",
  RemoveHostFiles: "removeHostFiles",
} as const;
export type ScmStepType = (typeof ScmStepType)[keyof typeof ScmStepType];

export const PlanActionType = {
  WriteFile: "writeFile",
  VerifyDefinition: "verifyDefinition",
  RemoveFile: "removeFile",
  Command: "command",
  Scm: "scm",
} as const;
export type PlanActionType = (typeof PlanActionType)[keyof typeof PlanActionType];

export const Scope = {
  User: "user",
  System: "system",
} as const;
export type Scope = (typeof Scope)[keyof typeof Scope];
export const RestartPolicy = {
  Never: "never",
  OnFailure: "onFailure",
  Always: "always",
} as const;
export type RestartPolicy = (typeof RestartPolicy)[keyof typeof RestartPolicy];
export const Operation = {
  Install: "install",
  Uninstall: "uninstall",
  Start: "start",
  Stop: "stop",
  Restart: "restart",
  Status: "status",
} as const;
export type Operation = (typeof Operation)[keyof typeof Operation];
export const BuiltinBackend = {
  Systemd: "systemd",
  Launchd: "launchd",
  Scm: "scm",
  Openrc: "openrc",
  Procd: "procd",
} as const;
export type BuiltinBackend = (typeof BuiltinBackend)[keyof typeof BuiltinBackend];
export const BackendSelection = { Auto: "auto", ...BuiltinBackend } as const;
export type BackendSelection = (typeof BackendSelection)[keyof typeof BackendSelection];
/** Custom registry names remain supported while built-ins receive editor completion. */
export type BackendName = BuiltinBackend | (string & {});
export const ErrorCode = {
  InvalidSpec: "INVALID_SPEC",
  Unsupported: "UNSUPPORTED",
  Unavailable: "UNAVAILABLE",
  PermissionDenied: "PERMISSION_DENIED",
  Conflict: "CONFLICT",
  Io: "IO",
  CommandFailed: "COMMAND_FAILED",
  NotFound: "NOT_FOUND",
  InvalidArguments: "INVALID_ARGUMENTS",
  InvalidHost: "INVALID_HOST",
  UnsafePath: "UNSAFE_PATH",
  ManagerFailed: "MANAGER_FAILED",
  Timeout: "TIMEOUT",
  BinaryNotFound: "BINARY_NOT_FOUND",
  InvalidProtocol: "INVALID_PROTOCOL",
  UnsupportedProtocolVersion: "UNSUPPORTED_PROTOCOL_VERSION",
  TransportError: "TRANSPORT_ERROR",
} as const;
export type ErrorCode = (typeof ErrorCode)[keyof typeof ErrorCode];
export type StructuredError = { code: ErrorCode; message: string };
export type BackendCandidate = { backend: BackendName; managerAvailable: boolean; reachable: boolean; canWrite: boolean; diagnostics: string[]; capabilities: { scopes: Scope[]; restartPolicies: RestartPolicy[]; supportsApply: boolean; experimental: boolean } };
export type ProbeData = { host: { os: HostPlatform; home: string; uid: number | null; elevated: boolean }; candidates: BackendCandidate[] };
export type ServiceStatus = { state: ServiceState; detail: string };
export type CommandCondition = { type: typeof CommandConditionType.LaunchdJobAbsent | typeof CommandConditionType.LaunchdJobPresent; domain: string; target: string; expectedPath: string };
export type ScmStep =
  | { type: typeof ScmStepType.VerifyOwnership; serviceName: string; directory: string; absentIsSuccess: boolean }
  | { type: typeof ScmStepType.EnsureProtectedDirectory; path: string; sddl: string }
  | { type: typeof ScmStepType.WriteSpec; path: string; content: string }
  | { type: typeof ScmStepType.CopyHost; source: string; destination: string }
  | { type: typeof ScmStepType.CreateService; serviceName: string; host: string; specPath: string; startup: Startup; description: string }
  | { type: typeof ScmStepType.ConfigureRecovery; restart: RestartPolicy; resetSecs: number; delaySecs: number }
  | { type: typeof ScmStepType.StopIfRunning | typeof ScmStepType.StartIfStopped; timeoutSecs: number }
  | { type: typeof ScmStepType.DeleteService | typeof ScmStepType.QueryStatus }
  | { type: typeof ScmStepType.RemoveHostFiles; directory: string; files: string[] };
export type PlanAction =
  | { type: typeof PlanActionType.WriteFile; path: string; content: string; makeExecutable: boolean }
  | { type: typeof PlanActionType.VerifyDefinition; path: string; expectedContent: string; absentIsSuccess: boolean; verifyContent: boolean }
  | { type: typeof PlanActionType.RemoveFile; path: string; expectedContent: string }
  | { type: typeof PlanActionType.Command; executable: string; args: string[]; condition: CommandCondition | null; queryStatus: boolean }
  | { type: typeof PlanActionType.Scm; step: ScmStep; onlyIfAbsent: boolean };
export type ServicePlan = { backend: BackendName; operation: Operation; serviceId: string; scope: Scope; actions: PlanAction[]; warnings: string[] };
export type ManageOptions = { backend?: BackendName | BackendSelection; dryRun?: boolean };
