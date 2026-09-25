import { loadMetadata, syncMetadata } from "./metadata.ts";
import { readFile, writeFile, mkdir, cp } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { root } from "./paths.ts";
import { run } from "./process.ts";
import { withTempDirectory } from "./temp.ts";
import { createZip } from "./archive.ts";

export const releaseTargets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl", "x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"] as const;
export async function packageRelease(target: string, requestedVersion?: string, base = root, metadataChecked = false): Promise<string> {
  if (!releaseTargets.includes(target as typeof releaseTargets[number])) throw new Error(`Unsupported release target: ${target}`);
  if (!metadataChecked) await syncMetadata(true, base);
  const version = loadMetadata(base).project.version!;
  if (requestedVersion && requestedVersion.replace(/^v/, "") !== version) throw new Error("Release tag/version must match project metadata version");
  const windows = target.includes("windows");
  const binaryNames = windows ? ["service-kit.exe", "service-kit-host.exe"] : ["service-kit"];
  const binaryPaths = binaryNames.map(name => join(base, "target", target, "release", name));
  return withTempDirectory("package", async (scratch) => {
    const basename = `service-kit-${version}-${target}`;
    const staging = join(scratch, basename);
    const dist = join(base, "dist");
    await mkdir(staging); await mkdir(dist, { recursive: true });
    for (let i = 0; i < binaryNames.length; i++) await cp(binaryPaths[i]!, join(staging, binaryNames[i]!));
    for (const file of ["README.md", "CHANGELOG.md", "LICENSE"]) await cp(join(base, file), join(staging, file));
    const archive = join(dist, `${basename}.${windows ? "zip" : "tar.gz"}`);
    if (windows) {
      await createZip(staging, archive);
    } else await run("tar", ["-czf", archive, "-C", scratch, basename]);
    const hash = createHash("sha256").update(await readFile(archive)).digest("hex");
    await writeFile(`${archive}.sha256`, `${hash}  ${archive.split(/[\\/]/).at(-1)}\n`);
    return archive;
  });
}
