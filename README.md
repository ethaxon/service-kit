# service-kit

Manage background programs as native operating-system services, with explicit backend selection and a reviewable plan before installation.

Supports systemd on Linux and launchd on macOS. Windows SCM uses a bundled Rust Service Host and is experimental. OpenRC and procd currently generate configuration plans only.

## Getting started

The project is unreleased. For now, build the CLI from this checkout with Rust:

```sh
cargo build --release -p service-kit-cli
./target/release/service-kit probe
./target/release/service-kit plan install --spec examples/service.json --backend launchd
```

Edit the example specification to use your program's absolute executable path; choose `systemd` for Linux. Probe and plan do not register services. Review the plan before using commands that change services.

The [user guide](docs/en/guide.md) covers service specifications, lifecycle commands and the Node wrapper. See [platform support and limitations](docs/en/architecture.md) before using experimental backends. Windows builds also require the bundled host; follow the guide for packaging details.

## Contributing

Development setup, checks and release workflows are in [CONTRIBUTING.md](CONTRIBUTING.md). Coding agents should read [AGENTS.md](AGENTS.md).

## License and changes

MIT. See the [changelog](CHANGELOG.md) for changes.

[English](README.md) | [中文](docs/zh/readme.md)
