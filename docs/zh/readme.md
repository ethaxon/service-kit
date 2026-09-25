# service-kit

将后台程序交给操作系统的原生服务管理器管理，支持明确选择后端，并在安装前查看执行计划。

支持 Linux systemd 和 macOS launchd。Windows SCM 使用随附的 Rust Service Host，目前为实验性实现；OpenRC 和 procd 暂时只生成配置计划。

## 开始使用

项目尚未发布。目前可以在仓库中使用 Rust 构建 CLI：

```sh
cargo build --release -p service-kit-cli
./target/release/service-kit probe
./target/release/service-kit plan install --spec examples/service.json --backend launchd
```

先将示例配置中的可执行文件改为自己的程序绝对路径；Linux 选择 `systemd` 后端。probe 和 plan 不会注册服务，执行修改服务的命令前请先检查计划。

[使用指南](guide.md)介绍服务配置、生命周期命令和 Node 包装器。使用实验性后端前请阅读[平台支持与限制](architecture.md)。Windows 还需要随附的 Host，打包方式见贡献指南。

## 参与开发

开发环境、检查和发布流程见[贡献指南](contributing.md)。编码 agent 应先阅读 [AGENTS.md](../../AGENTS.md)。

## 许可与变更

采用 MIT 许可，变更见[更新日志](changelog.md)。

[English](../en/readme.md) | [中文](readme.md)
