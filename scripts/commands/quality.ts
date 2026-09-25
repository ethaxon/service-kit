import { Effect } from "effect";
import { syncMetadata } from "../lib/metadata.ts";
import { run, runNode } from "../lib/process.ts";

export async function check(): Promise<void> {
  const checks = [
    () => syncMetadata(true),
    () => run("cargo", ["fmt", "--all", "--", "--check"]),
    () => run("cargo", ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
    () => runNode("node_modules/typescript/bin/tsc"),
  ];
  await Effect.runPromise(Effect.all(checks.map(check => Effect.tryPromise(async () => { await check(); })), { concurrency: "unbounded" }));
}

export async function test(): Promise<void> {
  await run("cargo", ["test", "--workspace", "--locked"]);
  await run("cargo", ["build", "-p", "service-kit-cli", "--locked"]);
  await run(process.execPath, ["--test", "scripts/lib/__tests__/tooling.test.ts", "sdk/node/index.test.ts"]);
}
