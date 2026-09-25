# Architecture and support

## Components

| Directory | Responsibility |
| --- | --- |
| `crates/core` | Service specifications, capabilities, backend candidates, strategies, plans and protocol types |
| `crates/backends` | Native manager probes, configuration rendering and plan execution |
| `crates/cli` | Figment configuration providers and JSON command interface |
| `crates/host` | Windows SCM host, child process lifetime and stop coordination |

Rust owns service behavior. The Node SDK invokes the CLI and exposes typed responses. Register custom backends through `BackendRegistry`; implement `Strategy` for application-specific selection. `PriorityStrategy` evaluates candidates in order and reports exclusion reasons. `FixedBackendStrategy` selects a named candidate. Backend names remain extensible strings.

The execution flow is specification → backend validation → plan → apply. Explicit backends permit offline plans. Automatic selection probes manager reachability, operation permissions and capabilities. Status uses read access; mutations require write access. Elevated user-scope operations are rejected so the home directory and session identity stay aligned.

Plans describe ownership checks, file operations, conditional commands, status queries and detailed SCM steps. Apply generates and executes the same actions. launchd bootstrap/bootout depend on whether the job is loaded; an existing job must refer to the expected definition path. Stored plans are review artifacts; operations accept the service specification.

## Platform support

| Backend | Scope | Restart policies | Implementation / verification level |
| --- | --- | --- | --- |
| systemd / Linux | user, system | never, onFailure, always | Lifecycle implemented; linked builds; real lifecycle pending |
| launchd / macOS | user, system | never, onFailure, always | Lifecycle implemented; read-only native probe exercised; lifecycle pending |
| SCM / Windows | system | never, onFailure | Experimental host and adapter; compile checked; native lifecycle pending |
| OpenRC / Linux | system | never | Configuration generation only |
| procd / Linux | system | never | Configuration generation only |

systemd uses the user or system manager and enables automatic services through target links. launchd uses `gui/UID` or the system bootstrap domain. KeepAlive implies an initial launch, so manual startup and stop with a restart policy are rejected. OpenRC currently supports arguments but omits environment and working-directory support.

## Windows host

SCM runs a bundled Rust host as LocalSystem. Each service keeps its specification, host binary and logs under `C:\ProgramData\ServiceKit\ID`, protected for SYSTEM and Administrators. Installation plans list directory ACLs, file placement, registration and recovery settings. Installation steps run when the managed SCM entry is absent.

The host creates the child suspended, assigns a kill-on-close Job Object and resumes execution. Programs may cooperate with the named event in `SERVICE_KIT_STOP_EVENT`; after the stop timeout, the host terminates the process tree. SCM handles restart-on-failure. stdout, stderr and host errors append to separate logs.

## Limitations

Definitions are written atomically, with content ownership checks. Repeated identical installation and absent uninstallation are idempotent. To change a service, uninstall with its original specification and install the new one.

A manager failure can leave completed steps in place; inspect the state before retrying. Transactional updates, hostile concurrent path changes, custom Windows service accounts, log rotation and alternate launchd domains require further work. Native integration environments are needed to verify lifecycle, Windows ACL, Job Object and recovery behavior.

[English](architecture.md) | [中文](../zh/architecture.md)
