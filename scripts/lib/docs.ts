import { lstat, mkdir, readlink, realpath, readdir, readFile, symlink } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { root } from "./paths.ts";
export const links = [
  ["docs/en/readme.md", "README.md", "file"],
  ["docs/en/changelog.md", "CHANGELOG.md", "file"],
  ["docs/en/contributing.md", "CONTRIBUTING.md", "file"],
  ["docsite/en", "docs/en", "dir"],
  ["docsite/zh", "docs/zh", "dir"],
  ["docsite/index.md", "docs/en/index.md", "file"],
] as const;
export async function syncLinks(base = root): Promise<void> {
  for (const [link, target, type] of links) {
    const path = join(base, link);
    await mkdir(dirname(path), { recursive: true });
    try { await lstat(path); continue; } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
    try { await symlink(relative(dirname(path), join(base, target)), path, type); }
    catch (error) { throw new Error(`Cannot create real relative symlink ${link}. On Windows enable Developer Mode or run elevated; use git core.symlinks=true. Junctions and copies are unsupported. ${String(error)}`); }
  }
}
async function sourceMarkdown(base: string, dir: string): Promise<string[]> {
  const result: string[] = [];
  for (const entry of await readdir(join(base, dir), { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) result.push(...await sourceMarkdown(base, path));
    else if (path.endsWith(".md")) result.push(path);
  }
  return result;
}
export async function checkLinks(base = root): Promise<void> {
  for (const [link, target] of links) {
    const path = join(base, link);
    const stat = await lstat(path);
    if (!stat.isSymbolicLink()) throw new Error(`${link} must be a real symlink; on Windows enable Developer Mode and checkout with git core.symlinks=true`);
    const actual = await readlink(path);
    const expected = relative(dirname(path), join(base, target));
    if (isAbsolute(actual) || actual !== expected) throw new Error(`${link} must use relative symlink target ${expected}`);
    if (await realpath(path) !== await realpath(join(base, target))) throw new Error(`Broken documentation link ${link}`);
  }
  // Detect accidental copied Markdown anywhere in docsite, including extra nested pages.
  async function inspect(dir: string, throughLink = false): Promise<void> {
    for (const entry of await readdir(join(base, dir))) {
      if (["node_modules", ".vitepress"].includes(entry)) continue;
      const path = join(dir, entry); const stat = await lstat(join(base, path));
      if (stat.isSymbolicLink()) {
        const target = await readlink(join(base, path));
        if (isAbsolute(target)) throw new Error(`Absolute docsite link ${path}`);
        const dest = await realpath(join(base, path));
        const docsRoot = await realpath(join(base, "docs"));
        if (!(dest.startsWith(docsRoot + sep) || (path === join("docsite", "en") && dest === await realpath(join(base, "docs/en"))))) throw new Error(`docsite link must point into docs: ${path}`);
      } else if (stat.isDirectory()) await inspect(path, throughLink);
      else if (path.endsWith(".md") && !throughLink) throw new Error(`Copied Markdown forbidden: ${path}`);
    }
  }
  await inspect("docsite");
  // Validate relative Markdown targets in canonical source files.
  for (const file of await sourceMarkdown(base, "docs")) {
    const canonical = await realpath(join(base, file));
    const body = await readFile(canonical, "utf8");
    for (const match of body.matchAll(/\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g)) {
      const raw = match[1]!;
      if (/^(?:[a-z]+:|#|\/)/i.test(raw)) continue;
      const target = decodeURIComponent(raw.split("#")[0]!);
      if (!target) continue;
      try { await realpath(resolve(dirname(canonical), target)); }
      catch { throw new Error(`Broken Markdown link ${file}: ${raw}`); }
    }
  }
}
