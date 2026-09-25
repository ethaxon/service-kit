# Contributor workflows

`mise.toml` manages the environment. Rust has one version source, `rust-toolchain.toml`, which mise reads with `[settings] idiomatic_version_file_enable_tools = ["rust"]`; do not duplicate it under `[tools]`. `justfile` is the human entry point; `scripts/dev-cli.ts` only parses and dispatches to `scripts/commands/`; shared implementations live in `scripts/lib/`. Local and CI work use this same entry point. Just recipes are grouped under `justfiles/`. It runs directly with native Node TypeScript, uses Optique parsing, Effect workflows and optional Inquirer selection. CI always supplies explicit arguments.

```sh
just setup
just check
just test
just build-docs
just check-docs
just build --target aarch64-apple-darwin
just package --target aarch64-apple-darwin
just dist --target aarch64-apple-darwin --release-tag 0.1.0
```

`dist` prepares checked local artifacts; only the GitHub tag workflow uploads them. `build --builder zigbuild` uses the optional mise cross environment: run `MISE_ENV=cross mise install`, then `MISE_ENV=cross just build --target TARGET --builder zigbuild`. Setup bootstraps mise, then runs the dependency-free Node setup workflow to install frozen pnpm dependencies and link documentation. Check runs Rust fmt/clippy with warnings denied, TypeScript typecheck and metadata validation. Test runs Rust behavior/protocol tests, builds the CLI, then node:test tooling/SDK tests. Docs validates links and performs a VitePress production build.

English README/CHANGELOG/CONTRIBUTING are canonical root files; docs/en references them through actual relative symlinks. All other documentation is canonical in docs/en or docs/zh. docsite is an independent VitePress app whose en/zh directories symlink to docs; its root index symlinks to docs/en/index.md. Do not copy Markdown, use hard links or junctions. Validation checks link type, relative targets, broken links and accidental copied Markdown.

On Windows enable Developer Mode or use an elevated checkout, set `git config core.symlinks true` before checkout and rerun setup. A checkout that produced text files in place of links must be repaired with proper symlink support; setup refuses to overwrite these files. Permission failures explain the required action and do not silently create junctions.

Temporary test fixtures, packaging staging and agent scratch files use ignored repository `temp/` and clean up afterward. Atomic service writes necessarily stage beside their destination for same-filesystem persistence. Build caches stay in target/node_modules; final local release archives go to dist. Default tests use temporary files and offline plans, never real service registration. Real lifecycle/elevation tests require a dedicated environment and explicit user opt-in; no such tests run automatically.


## Releases

Release targets:

| Runner family | Targets |
| --- | --- |
| Linux | x86_64/aarch64 unknown-linux-gnu and unknown-linux-musl |
| macOS | x86_64/aarch64 apple-darwin |
| Windows | x86_64/aarch64 pc-windows-msvc |

The test workflow runs the same check/test/docs workflows on Linux, macOS and Windows. Tag builds use Linux cargo-zigbuild + Zig for GNU/musl, macOS SDK/native runner cross-architecture builds and Windows MSVC native-runner targets. Running a binary on another architecture is separate from compiling it; CI does not claim service lifecycle coverage.

`just build --target TRIPLE [--builder zigbuild]` makes a release build. For Linux cross builds, `MISE_ENV=cross mise install` enables the pinned Zig/cargo-zigbuild tools from `mise.cross.toml`; invoke the build with `MISE_ENV=cross` too. `just package --target TRIPLE` requires all binaries to already exist. Packages are named `service-kit-VERSION-TARGET.tar.gz` or `.zip`, include CLI, Windows host when needed, README, CHANGELOG and LICENSE, and have per-archive SHA-256 files. Missing binaries fail before any archive is emitted. Staging uses ignored repository `temp/` and is removed afterward.

A `vVERSION` tag must match Cargo workspace version. Release workflow collects all assets, writes aggregate SHA256SUMS and uploads with GitHub CLI. Only tag jobs have release write permissions.

The separate Pages workflow builds docsite from docs symlinks and deploys its output. Repository Pages must use GitHub Actions as source. The site has base `/`; update that setting for a renamed repository or custom domain.


## Project metadata

Edit `service-kit-metadata.toml`, then run `just sync-metadata`. Cargo and Node manifests are projections; checks and packaging reject drift. VitePress reads metadata directly. Dependency and toolchain versions remain in their dedicated manifests.

[English](CONTRIBUTING.md) | [中文](docs/zh/contributing.md)

