import test from "node:test";
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { withTempDirectory } from "../../scripts/lib/temp.ts";
import { join, resolve } from "node:path";
import { ServiceKit, BuiltinBackend, ErrorCode, Operation, Scope, Startup, RestartPolicy, decodeEnvelope } from "./index.ts";
test("protocol rejects malformed output, unsupported versions and exit mismatches", () => {
  assert.throws(() => decodeEnvelope("log\n{}", 0), { code: ErrorCode.InvalidProtocol });
  assert.throws(() => decodeEnvelope('{"protocolVersion":2}', 0), { code: ErrorCode.UnsupportedProtocolVersion });
  assert.throws(() => decodeEnvelope('{"protocolVersion":1,"success":true,"data":{},"error":null}', 1), { code: ErrorCode.InvalidProtocol });
  assert.throws(() => decodeEnvelope('{"protocolVersion":1,"success":false,"data":null,"error":{"code":"UNSUPPORTED","message":"scope"}}', 1), { code: ErrorCode.Unsupported, message: "scope" });
});
test("actual Rust CLI supports offline plans through the SDK", async () => {
  const client = new ServiceKit({ binary: resolve("target/debug", process.platform === "win32" ? "service-kit.exe" : "service-kit") });
  await withTempDirectory("smoke", async (temp) => {
    const path = join(temp, "spec.json");
    await writeFile(path, JSON.stringify({ id: "sdk-smoke", executable: "/bin/echo", scope: Scope.System, startup: Startup.Manual, restart: RestartPolicy.Never }));
    const plan = await client.plan(Operation.Install, path, { backend: BuiltinBackend.Systemd });
    assert.equal(plan.backend, BuiltinBackend.Systemd);
    await writeFile(path, "{}"); await assert.rejects(client.plan(Operation.Install, path, { backend: BuiltinBackend.Systemd }), { code: ErrorCode.InvalidSpec });
  });
});
