import { checkLinks, syncLinks } from "../lib/docs.ts";
import { runNode } from "../lib/process.ts";

export async function buildDocs(): Promise<void> {
  await checkLinks();
  await runNode("docsite/node_modules/vitepress/bin/vitepress.js", ["build", "docsite"]);
}

export async function syncDocs(): Promise<void> {
  await syncLinks();
}
