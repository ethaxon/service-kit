import { loadMetadata } from "../../scripts/lib/metadata.ts";
import { defineConfig } from "vitepress";
import { siteDocLink } from "../../scripts/lib/doc-links.ts";
const { project, docs } = loadMetadata();
const pages = ["readme", "guide", "architecture", "contributing", "changelog"];
const labels = { en: ["Overview", "User guide", "Architecture and support", "Contributing", "Changelog"], zh: ["概览", "使用指南", "架构与支持", "贡献指南", "更新日志"] };
export default defineConfig({
  title: project.display_name, description: project.description, base: docs.base,
  cleanUrls: true,
  vite: { resolve: { preserveSymlinks: true } },
  markdown: {
    config(markdown) {
      const fallback = markdown.renderer.rules.link_open;
      markdown.renderer.rules.link_open = (tokens, index, options, env, self) => {
        const token = tokens[index]!;
        const href = token.attrGet("href");
        if (href) token.attrSet("href", siteDocLink(href, String(env.path ?? env.realPath ?? "")));
        return fallback ? fallback(tokens, index, options, env, self) : self.renderToken(tokens, index, options);
      };
    },
  },
  locales: {
    root: { label: "English", lang: "en", themeConfig: { nav: [{ text: "Guide", link: "/en/guide" }], sidebar: [{ text: "Documentation", items: pages.map((page, index) => ({ text: labels.en[index]!, link: `/en/${page}` })) }] } },
    zh: { label: "简体中文", lang: "zh-CN", themeConfig: { nav: [{ text: "指南", link: "/zh/guide" }], sidebar: [{ text: "文档", items: pages.map((page, index) => ({ text: labels.zh[index]!, link: `/zh/${page}` })) }] } },
  },
  themeConfig: { socialLinks: [{ icon: "github", link: project.repository_url! }] },
});
