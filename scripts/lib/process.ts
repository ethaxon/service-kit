import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { join } from "node:path";
import { root } from "./paths.ts";

const exec = promisify(execFile);

export async function run(program: string, args: string[], cwd = root): Promise<string> {
  console.error(`> ${program} ${args.join(" ")}`);
  try {
    const result = await exec(program, args, {
      cwd, maxBuffer: 16 * 1024 * 1024, env: process.env,
      windowsHide: true, shell: false,
    });
    if (result.stderr) process.stderr.write(result.stderr);
    if (result.stdout) process.stdout.write(result.stdout);
    return result.stdout;
  } catch (error) {
    const failure = error as Error & { stdout?: string; stderr?: string };
    if (failure.stdout) process.stdout.write(failure.stdout);
    if (failure.stderr) process.stderr.write(failure.stderr);
    throw error;
  }
}

export function runNode(script: string, args: string[] = []) {
  return run(process.execPath, [join(root, script), ...args]);
}

export async function hostTarget(): Promise<string> {
  const output = await run("rustc", ["-vV"]);
  const target = /^host: (.+)$/m.exec(output)?.[1];
  if (!target) throw new Error("rustc did not report its host target");
  return target;
}
