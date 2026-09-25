# 贡献与发布

## 贡献指南


Rust 版本只在 `rust-toolchain.toml` 声明，mise 通过 `[settings] idiomatic_version_file_enable_tools = ["rust"]` 读取；不在 `[tools]` 再声明 rust。所有检查通过 `mise exec` 使用项目工具链。

`justfile` 为人类入口，按职责导入 `justfiles/`。`scripts/dev-cli.ts` 只解析和分发，具体工作流放在 `scripts/commands/`，可复用实现放在 `scripts/lib/`。使用 Node 原生 TypeScript、Optique、Effect、可选 Inquirer 和 node:test。CI 显式传参，不启动交互选择。

```sh
just setup
just check
just test
just link-docs
just check-docs
just build-docs
just build --target aarch64-apple-darwin
just package --target aarch64-apple-darwin
```

`docs/en` 与 `docs/zh` 是文档真源；英语 README/CHANGELOG/CONTRIBUTING 根目录为唯一真源，docs/en 通过相对 symlink 引用。docsite 独立应用通过相对 symlink 消费 docs，禁止复制内容、junction 和 hardlink。Windows 需要 Developer Mode 或足够的 symlink 权限，克隆时设置 `core.symlinks=true`；工具不会用复制替代软链接。

临时文件和打包/测试夹具放在仓库 `temp/` 并清理，构建缓存保留在 target/node_modules，发行包输出 dist。服务原子文件写入必须在目标同一目录暂存，以保证同文件系统原子落盘。默认测试不注册任何真实服务；真实生命周期验证需独立且明确启用的环境。

## 构建与发布

| 构建环境 | 目标 |
| --- | --- |
| Linux | x86_64/aarch64 GNU 和 musl |
| macOS | x86_64/aarch64 Darwin |
| Windows | x86_64/aarch64 MSVC |

本机和 CI 共用开发 CLI。Linux 发布可用 cargo-zigbuild + Zig，macOS/Windows 使用原生 runner 和相应 SDK。真实服务生命周期在单独启用的平台集成环境验证。

使用 `MISE_ENV=cross mise install` 安装 mise.cross.toml 中固定版本的 Zig/cargo-zigbuild，Linux 交叉构建时也设置 `MISE_ENV=cross`。`just build --target TRIPLE [--builder zigbuild]` 编译；`just package --target TRIPLE` 要求全部二进制已经存在。归档以版本和 target 命名，Windows 同包提供 CLI/Host，并包含 README、CHANGELOG、LICENSE 及 SHA-256。暂存在仓库 temp 并清理；缺少二进制直接失败。

`vVERSION` 标签必须与 Cargo 工作区版本一致。GitHub Actions 负责构建、收集产物及校验和，最后上传 GitHub Release。

文档站为独立 VitePress app，由 Pages 工作流部署，默认 base 为 `/`。仓库需启用 GitHub Actions 作为 Pages 来源。本地可运行 `just build-docs` 验证。


## 项目元数据

修改 `service-kit-metadata.toml` 后运行 `just sync-metadata`。Cargo 和 Node 清单为投影，检查与打包会拒绝不一致；VitePress 直接读取元数据。依赖和工具链版本仍由各自清单管理。

[English](../en/contributing.md) | [中文](contributing.md)

