import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tempRoot } from "./paths.ts";

export async function withTempDirectory<T>(prefix: string, work: (path: string) => Promise<T>): Promise<T> {
  await mkdir(tempRoot, { recursive: true });
  const directory = await mkdtemp(join(tempRoot, `${prefix}-`));
  try { return await work(directory); }
  finally { await rm(directory, { recursive: true, force: true }); }
}
