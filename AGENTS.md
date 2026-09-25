# Contributor instructions

## Working rules

- Preserve unrelated working-tree changes. Never reset or clean the repository; leave commits and publishing to explicit user requests.
- Use the user's language in conversation and English for code, identifiers and comments.
- Never set `autoResolutionMs` for interactive questions. Wait for an explicit response.
- Follow the engineering conventions established in `../securitydept` and `../Android-Credential-Provider-Fixer`. Current task instructions take precedence over historical differences.

## Architecture and scope

- Rust owns all service-management behavior. Node and future SDKs only invoke the versioned Rust CLI protocol.
- Keep backend, probe, selection strategy, plan and apply separate. Reject unsupported semantics explicitly.
- Use concise source directories: `crates/core`, `crates/backends`, `crates/cli`, `crates/host`. Cargo package names may retain the `service-kit-` prefix.
- Windows uses SCM with the bundled Rust Host; never silently fall back to Task Scheduler. WASI and NAPI are outside the initial scope.

## Toolchain and automation

- Project metadata lives in `service-kit-metadata.toml`; run `just sync-metadata` after editing. Do not independently change projected Cargo/Node metadata.

- Rust's sole version source is `rust-toolchain.toml`. Enable mise's `[settings] idiomatic_version_file_enable_tools = ["rust"]`; do not duplicate the version in `tools.rust`.
- Run verification under `mise exec`. Keep optional Zig/cargo-zigbuild versions in `mise.cross.toml`; use native macOS/Windows runners for release builds.
- `justfile` imports recipes grouped by responsibility under `justfiles/` and provides the human entry point.
- `scripts/dev-cli.ts` only parses and dispatches. Put workflows in `scripts/commands/` and reusable implementations in `scripts/lib/`; CI invokes these same workflows.
- Use native Node TypeScript, Effect, `@optique/core`, optional `@inquirer/prompts`, and `node:test`. Keep archive creation in the TypeScript workflow rather than a separate platform shell script.
- For TypeScript enum-like definitions, prefer a const object and a same-name value-union type: `const A = { Example: "example" } as const; type A = (typeof A)[keyof typeof A];`. Use this pattern for new definitions and migrate existing enums where semantics permit, providing both runtime values and a type compatible with type-stripping-only execution. Avoid TypeScript `enum` and `const enum` in code executed through native type stripping.
- Agent scratch files, test fixtures and packaging staging use ignored repository `temp/`, with cleanup. Build caches stay in their conventional directories; atomic service writes stage beside the destination.

## Documentation

- Use relative links for repository content. Keep external URLs only for external references or the repository's social link.
- Use lowercase filenames consistently under `docs/en/` and `docs/zh/`, with matching names and language links. Prefer a few cohesive guides over a page per small topic.
- Root `README.md`, `CHANGELOG.md` and `CONTRIBUTING.md` are the sole English convention-document sources. The lowercase files in `docs/en/` are relative symlinks to them; Chinese source files live under `docs/zh/`.
- `docsite/` is an independent VitePress app for GitHub Pages. Its Markdown content resolves through real relative symlinks to `docs/`; never copy content or use junctions/hard links.
- Resolve source-relative links correctly when rendering symlinked documents. Keep dead-link checks enabled, and report Windows symlink permission failures rather than substituting copies.
- Keep current usage in the user guide, architecture/support in the architecture guide, and contributor/release workflows in CONTRIBUTING. Historical changes belong in CHANGELOG.

## Validation and reporting

- Run formatting, relevant lint/type checks, tests and affected builds; fix failures before reporting completion.
- Default tests must not install or change real services. Use fixtures/offline plans; real lifecycle tests require explicit opt-in integration environments.
- In conversation reports, distinguish implemented behavior, compile-only checks, linked builds and real-platform execution.
- Test project behavior and protocol contracts; avoid testing third-party implementation details.
- Validate at external-input, deserialization, process and filesystem boundaries. Avoid repeating pure checks in internal calls; retain checks across filesystem mutations.
- Long-lived documentation describes current behavior and limitations, not task logs, test counts or defensive disclaimers.

The executable task is retained at [.codex/tasks/initialize.md](.codex/tasks/initialize.md).
