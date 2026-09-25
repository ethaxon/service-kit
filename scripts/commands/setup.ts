import { syncLinks } from "../lib/docs.ts";
import { run } from "../lib/process.ts";

export async function setup(): Promise<void> {
  await run("mise", ["exec", "--", "pnpm", "install", "--frozen-lockfile"]);
  await syncLinks();
}
