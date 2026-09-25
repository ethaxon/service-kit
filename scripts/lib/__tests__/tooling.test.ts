import test from "node:test";
import assert from "node:assert/strict";
import { mkdir, writeFile, readFile, rm, unlink, readlink } from "node:fs/promises";
import { join } from "node:path";
import { packageRelease } from "../release.ts";
import { withTempDirectory } from "../temp.ts";
import { links, syncLinks, checkLinks } from "../docs.ts";
test("documentation links are relative real symlinks; copies and broken links fail", async () => {
  await withTempDirectory("check-docs", async (temp) => {
    for (const dir of ["docs/en", "docs/zh", "docsite"]) await mkdir(join(temp, dir), { recursive: true });
    for (const file of ["README.md", "CHANGELOG.md", "CONTRIBUTING.md", "docs/en/index.md"]) await writeFile(join(temp, file), "# Test\n");
    await syncLinks(temp); await checkLinks(temp);
    await writeFile(join(temp, "README.md"), "[Guide](docs/zh/guide.md)\n");
    await writeFile(join(temp, "docs/zh/guide.md"), "# Guide\n");
    await checkLinks(temp);
    await rm(join(temp, "docs/zh/guide.md"));
    await assert.rejects(checkLinks(temp), /Broken Markdown link/);
    await writeFile(join(temp, "README.md"), "# Test\n");
    for (const [link] of links) assert.ok(!(await readlink(join(temp, link))).startsWith(temp));
    await writeFile(join(temp, "docsite/copied.md"), "copy");
    await assert.rejects(checkLinks(temp), /Copied Markdown/);
    await rm(join(temp, "docsite/copied.md"));
    await unlink(join(temp, "docs/en/readme.md")); await writeFile(join(temp, "docs/en/readme.md"), "copy");
    await assert.rejects(checkLinks(temp), /real symlink/);
    await unlink(join(temp, "docs/en/readme.md")); await syncLinks(temp);
    await rm(join(temp, "docs/en/index.md")); await assert.rejects(checkLinks(temp));
  });
});
test("packaging fails on unknown target, mismatched version and missing binaries", async () => {
  await assert.rejects(packageRelease("unknown"), /Unsupported release target/);
  await assert.rejects(packageRelease("aarch64-apple-darwin", "999.0.0"), /version/);
  await withTempDirectory("package-missing", async (temp) => {
    const { root } = await import("../paths.ts");
    await writeFile(join(temp, "service-kit-metadata.toml"), await readFile(join(root, "service-kit-metadata.toml")));
    await assert.rejects(packageRelease("aarch64-pc-windows-msvc", undefined, temp, true), /ENOENT/);
  });
});

test("metadata synchronization detects drift and preserves unrelated manifest fields", async () => {
  const { syncMetadata } = await import("../metadata.ts");
  const { root } = await import("../paths.ts");
  await withTempDirectory("metadata", async (temp) => {
    for (const file of ["service-kit-metadata.toml", "docsite/public/CNAME", "Cargo.toml", "package.json", "sdk/node/package.json", "docsite/package.json"]) {
      await mkdir(join(temp, file, ".."), { recursive: true });
      await writeFile(join(temp, file), await readFile(join(root, file)));
    }
    await syncMetadata(true, temp);
    const path = join(temp, "service-kit-metadata.toml");
    await writeFile(path, (await readFile(path, "utf8")).replace('version = "0.1.0"', 'version = "0.2.0"'));
    await assert.rejects(syncMetadata(true, temp), /Metadata drift/);
    await syncMetadata(false, temp);
    await syncMetadata(true, temp);
    const pkg = JSON.parse(await readFile(join(temp, "sdk/node/package.json"), "utf8"));
    assert.equal(pkg.version, "0.2.0");
    assert.equal(pkg.exports, "./index.ts");
  });
});
