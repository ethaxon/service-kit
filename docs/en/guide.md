# User guide

## Quickstart

Build the CLI with `cargo build --release -p service-kit-cli`. Development setup is covered in [Contributing](contributing.md). Inspect available managers and generate an installation plan:

```sh
mise exec -- cargo run -p service-kit-cli -- probe --scope user
mise exec -- cargo run -p service-kit-cli -- plan install --spec examples/service.json --backend launchd
```

Use `--backend systemd` for offline Linux rendering. On Windows use a service spec with an absolute drive path, system scope and backend `scm`. Offline rendering with an explicit backend works across hosts. Auto selection requires a reachable, permitted execution backend.

Before a real install, replace the example with the actual long-running executable, arguments and environment. `install --spec FILE --backend NAME --dry-run` returns a plan; omitting `--dry-run` changes the real host. System mutations require elevation; status queries use read access. Installation registers/enables the service but does not explicitly start it; launchd RunAtLoad/KeepAlive can start it on bootstrap. Start explicitly with `start --spec FILE --backend NAME`.

Use the same unchanged spec for status and uninstall; changed/unrelated files are rejected. To change a service, uninstall with its old spec first. This initial release intentionally has no in-place update.

## ServiceSpec

`--spec FILE` loads TOML (`.toml`), JSON (`.json`) or YAML (`.yaml`/`.yml`) through Figment providers, selected by extension (case-insensitive). All formats use the same fields and validation. Unknown extensions, malformed configuration and missing files fail explicitly. No implicit environment overrides or file discovery are enabled. CLI responses remain JSON.

```json
{
  "id": "example.worker",
  "executable": "/opt/example/worker",
  "args": ["--message", "literal $HOME; no shell"],
  "environment": { "MODE": "production" },
  "workingDirectory": "/opt/example",
  "description": "Example worker",
  "scope": "user",
  "startup": "automatic",
  "restart": "onFailure",
  "backendOptions": { "systemd": { "afterNetwork": true } }
}
```

`id` is 1–128 ASCII alphanumeric/period/underscore/hyphen characters, starts alphanumeric. Internal manager names add `service-kit-` (systemd/SCM/init scripts) or `service-kit.` (launchd). `executable` and optional `workingDirectory` must be absolute, without parent (`..`) path components or control characters. Unix and Windows path syntax must match the selected backend. UNC paths and Windows user scope are unsupported.

`args`, `environment`, `description` and `backendOptions` default empty; `workingDirectory` is optional. Scope is `user|system`, startup is `manual|automatic`, restart is `never|onFailure|always`. No implicit default selects these behaviors. Unknown fields are errors. Environment names follow `[A-Za-z_][A-Za-z0-9_]*`; values/args/description cannot contain control characters. Windows rejects duplicate case-insensitive environment keys and the reserved host stop-event variable.

Supported typed backend options:

- `systemd: { afterNetwork: boolean }` adds ordering and wants network-online.target.
- `launchd: { processType: "Background|Standard|Interactive|Adaptive" }` selects a supported ProcessType.
- `scm: { stopTimeoutSecs: 1..300 }` configures the cooperative stop wait (default 20 seconds).

Options for a different backend are rejected. Arbitrary native configuration injection is unavailable. Secrets in specs and plans remain visible to the caller; manage file access accordingly. Unix generated definitions are owner-readable, Windows configuration/logs live in the protected host directory.

OpenRC currently rejects environment and workingDirectory; both OpenRC/procd accept restart=never only. launchd manual startup requires restart=never because KeepAlive implies an initial launch. SCM rejects restart=always. These restrictions are errors rather than silent degradation.

## CLI and JSON protocol v1

```text
service-kit probe [--scope user|system]
service-kit plan install|uninstall|start|stop|restart|status --spec FILE [--backend auto|NAME] [--priority systemd,launchd,scm]
service-kit install|uninstall|start|stop|restart|status --spec FILE [--backend auto|NAME] [--priority systemd,launchd,scm] [--dry-run]
```

Normal operations always produce one JSON envelope on stdout. Human help/version are conventional text. Diagnostics go to stderr. Any failed operation or invalid arguments exit nonzero. A successful probe can include excluded candidates: inspect each candidate, not only envelope success.

```json
{"protocolVersion":1,"success":true,"data":{"backend":"systemd"},"error":null}
```

```json
{"protocolVersion":1,"success":false,"data":null,"error":{"code":"UNSUPPORTED","message":"scope/restart unsupported"}}
```

Stable codes: `INVALID_SPEC`, `UNSUPPORTED`, `UNAVAILABLE`, `PERMISSION_DENIED`, `CONFLICT`, `IO`, `COMMAND_FAILED`, `NOT_FOUND`, `INVALID_ARGUMENTS`, `INVALID_HOST`, `UNSAFE_PATH`, `MANAGER_FAILED`, `TIMEOUT`. Status states: `running`, `stopped`, `failed`, `notInstalled`, `unknown`. Manager output is included in status detail; transient states may be reported unknown. Read `state` to determine whether the service is running.

Plan data includes backend, operation, serviceId, scope, actions and warnings. Action types are verifyDefinition/writeFile/removeFile/command/scm, with camelCase fields. Command conditions, status queries and SCM steps are included. Apply regenerates and executes this plan from the spec.

Native Node usage (Node with TypeScript stripping):

```ts
import { ServiceKit, Scope, Operation, BuiltinBackend } from "./sdk/node/index.ts";
const client = new ServiceKit({ binary: "/absolute/path/service-kit" });
const probes = await client.probe(Scope.User);
const plan = await client.plan(Operation.Install, "./service.toml", { backend: BuiltinBackend.Launchd });
```

Discovery uses an explicit override, `SERVICE_KIT_BINARY`, SDK-local bin, then PATH. The wrapper uses spawn without a shell, bounded output/timeouts, validates protocolVersion and propagates Rust errors as ServiceKitError. Transport errors use `TRANSPORT_ERROR`, missing executables `BINARY_NOT_FOUND`, invalid envelopes `INVALID_PROTOCOL`, incompatible versions `UNSUPPORTED_PROTOCOL_VERSION`.

`manageWithSelector(operation, specPath, scope, callback, { dryRun })` lets an SDK caller inspect probes and supply an explicit backend. The callback owns its selection policy; Rust verifies semantics and host availability again during real apply.

[English](guide.md) | [中文](../zh/guide.md)

