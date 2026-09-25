/** Thin transport only. Rust owns validation, probes, selection and service behavior. */
import { execFile } from "node:child_process";
import { access } from "node:fs/promises";
import { constants } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { promisify } from "node:util";
const execute = promisify(execFile);
export * from "./types.ts";
import { BackendSelection, ErrorCode, Scope } from "./types.ts";
import type { StructuredError, ProbeData, BackendName, Operation, ServicePlan, ServiceStatus, ManageOptions } from "./types.ts";
export class ServiceKitError extends Error {
  readonly code: ErrorCode;
  constructor(error: StructuredError) { super(error.message); this.name = "ServiceKitError"; this.code = error.code; }
}
export type ClientOptions = { binary?: string; timeoutMs?: number };
export async function discoverBinary(override?: string): Promise<string> {
  const name = process.platform === "win32" ? "service-kit.exe" : "service-kit";
  if (override ?? process.env.SERVICE_KIT_BINARY) return resolve((override ?? process.env.SERVICE_KIT_BINARY)!);
  for (const directory of [join(import.meta.dirname, "bin"), ...(process.env.PATH ?? "").split(delimiter)]) {
    const candidate = resolve(directory, name);
    try { await access(candidate, process.platform === "win32" ? constants.F_OK : constants.X_OK); return candidate; } catch { /* continue search */ }
  }
  throw new ServiceKitError({ code: ErrorCode.BinaryNotFound, message: "Install service-kit or set SERVICE_KIT_BINARY to its absolute executable path" });
}
export function decodeEnvelope<T>(stdout: string, exitCode: number): T {
  let value: unknown;
  try { value = JSON.parse(stdout); } catch { throw new ServiceKitError({ code: ErrorCode.InvalidProtocol, message: "CLI stdout is not a JSON envelope" }); }
  if (!value || typeof value !== "object") throw new ServiceKitError({ code: ErrorCode.InvalidProtocol, message: "Invalid CLI envelope" });
  const v = value as Record<string, unknown>;
  if (v.protocolVersion !== 1) throw new ServiceKitError({ code: ErrorCode.UnsupportedProtocolVersion, message: `Unsupported CLI protocol ${String(v.protocolVersion)}` });
  if (v.success === false && v.error && typeof v.error === "object") {
    const error = v.error as Record<string, unknown>;
    if (typeof error.code === "string" && typeof error.message === "string") throw new ServiceKitError({ code: error.code as ErrorCode, message: error.message });
  }
  if (v.success !== true || exitCode !== 0 || !Object.hasOwn(v, "data") || v.error !== null) throw new ServiceKitError({ code: ErrorCode.InvalidProtocol, message: "CLI exit status/envelope mismatch" });
  return v.data as T;
}
export class ServiceKit {
  readonly options: ClientOptions;
  constructor(options: ClientOptions = {}) { this.options = options; }
  async invoke<T = unknown>(args: string[]): Promise<T> {
    const binary = await discoverBinary(this.options.binary);
    let stdout: string;
    let exitCode = 0;
    try {
      const result = await execute(binary, args, { shell: false, windowsHide: true, timeout: this.options.timeoutMs ?? 30_000, maxBuffer: 4 * 1024 * 1024 });
      stdout = result.stdout;
    } catch (cause) {
      const e = cause as Error & { code?: number | string; stdout?: string; killed?: boolean };
      if (typeof e.code !== "number" || !e.stdout || e.killed) throw new ServiceKitError({ code: ErrorCode.TransportError, message: e.message });
      stdout = e.stdout; exitCode = e.code;
    }
    return decodeEnvelope<T>(stdout, exitCode);
  }
  probe(scope: Scope = Scope.User): Promise<ProbeData> { return this.invoke(["probe", "--scope", scope]); }
  plan(operation: Operation, spec: string, options: Omit<ManageOptions, "dryRun"> = {}): Promise<ServicePlan> {
    return this.invoke(["plan", operation, "--spec", resolve(spec), "--backend", options.backend ?? BackendSelection.Auto]);
  }
  manage(operation: Operation, spec: string, options: ManageOptions & { dryRun: true }): Promise<ServicePlan>;
  manage(operation: Operation, spec: string, options?: ManageOptions & { dryRun?: false }): Promise<ServiceStatus>;
  manage(operation: Operation, spec: string, options: ManageOptions): Promise<ServicePlan | ServiceStatus>;
  manage(operation: Operation, spec: string, options: ManageOptions = {}): Promise<ServicePlan | ServiceStatus> {
    return this.invoke([operation, "--spec", resolve(spec), "--backend", options.backend ?? BackendSelection.Auto, ...(options.dryRun ? ["--dry-run"] : [])]);
  }
  async manageWithSelector(operation: Operation, spec: string, scope: Scope, select: (probe: ProbeData) => BackendName | Promise<BackendName>, options: Pick<ManageOptions, "dryRun"> = {}): Promise<ServicePlan | ServiceStatus> {
    return this.manage(operation, spec, { ...options, backend: await select(await this.probe(scope)) });
  }
}
