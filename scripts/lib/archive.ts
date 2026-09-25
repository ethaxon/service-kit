import { readFile, readdir, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { zipSync, type Zippable } from "fflate";

/** Package the flat, controlled release staging directory on any host. */
export async function createZip(source: string, destination: string): Promise<void> {
  const files: Zippable = {};
  for (const entry of await readdir(source, { withFileTypes: true })) {
    if (!entry.isFile()) throw new Error(`Unexpected release entry: ${entry.name}`);
    files[`${basename(source)}/${entry.name}`] = await readFile(join(source, entry.name));
  }
  await writeFile(destination, zipSync(files));
}
