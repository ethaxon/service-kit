import { loadMetadata } from "./metadata.ts";
import { realpathSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { root } from "./paths.ts";

/** Translate canonical source-relative links to their documentation-site routes. */
export function siteDocLink(href: string, source: string): string {
  if (!source || /^(?:[a-z]+:|#|\/)/i.test(href)) return href;
  const split = href.search(/[?#]/);
  const path = split < 0 ? href : href.slice(0, split);
  const suffix = split < 0 ? "" : href.slice(split);
  const canonical = realpathSync(isAbsolute(source) ? source : resolve(root, "docsite", source));
  const target = relative(root, resolve(dirname(canonical), decodeURIComponent(path))).split(sep).join("/");
  if (target === "AGENTS.md") return `${loadMetadata().project.repository_url}/blob/main/AGENTS.md${suffix}`;
  const convention = /^(README|CHANGELOG|CONTRIBUTING)\.md$/.exec(target);
  if (convention) return `/en/${convention[1]!.toLowerCase()}${suffix}`;
  const document = /^docs\/(en|zh)\/([^/]+)\.md$/.exec(target);
  if (document) return `/${document[1]}/${document[2] === "index" ? "" : document[2]}${suffix}`;
  return href;
}
