import { readFileSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { parse } from "smol-toml";
import { root } from "./paths.ts";

export function loadMetadata(base = root) {
  const data = parse(readFileSync(join(base, "service-kit-metadata.toml"), "utf8"));
  const project = data.project as Record<string, string>;
  const docs = data.docs as Record<string, string>;
  const packages = data.node_package as { manifest: string }[];
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(project.version!)) throw new Error("Invalid project.version");
  return { project, docs, packages };
}

export async function syncMetadata(check = false, base = root): Promise<void> {
  const { project, docs, packages } = loadMetadata(base);
  const updates: [string, string][] = [];
  const cargo = await readFile(join(base, "Cargo.toml"), "utf8");
  const nextCargo = cargo.replace(/(\[workspace\.package\]\n)([\s\S]*?)(?=\n\[|$)/, (_, header: string, body: string) => {
    for (const [key, value] of Object.entries({ version: project.version, license: project.license, repository: project.repository_url, description: project.description })) {
      const line = `${key} = ${JSON.stringify(value)}`;
      const pattern = new RegExp(`^${key} = .*`, "m");
      body = pattern.test(body) ? body.replace(pattern, line) : `${body.trimEnd()}\n${line}\n`;
    }
    return header + body;
  });
  updates.push(["Cargo.toml", nextCargo]);
  for (const { manifest } of packages) {
    const data = JSON.parse(await readFile(join(base, manifest), "utf8"));
    Object.assign(data, { version: project.version, description: project.description, license: project.license, repository: project.repository_url });
    updates.push([manifest, JSON.stringify(data, null, 2) + "\n"]);
  }
  updates.push(["docsite/public/CNAME", docs.cname + "\n"]);
  for (const [file, next] of updates) {
    if (await readFile(join(base, file), "utf8") === next) continue;
    if (check) throw new Error(`Metadata drift in ${file}; run just sync-metadata`);
    await writeFile(join(base, file), next);
  }
}
