# 架构与支持

## 组件

| 目录 | 职责 |
| --- | --- |
| `crates/core` | 服务配置、能力、后端候选、策略、计划与协议类型 |
| `crates/backends` | 原生管理器探测、配置生成和计划执行 |
| `crates/cli` | Figment 配置 provider 和 JSON 命令接口 |
| `crates/host` | Windows SCM Host、子进程生命周期与停止协调 |

Rust 负责服务行为，Node SDK 调用 CLI 并提供类型化响应。通过 `BackendRegistry` 注册自定义后端，通过 `Strategy` 实现选择策略。`PriorityStrategy` 按顺序检查候选并报告排除原因，`FixedBackendStrategy` 选择指定候选。后端名使用可扩展的字符串。

执行流程为配置 → 后端校验 → 计划 → apply。指定后端可以生成离线计划；自动选择会探测管理器可达性、当前操作权限和能力。状态查询使用读权限，修改操作需要写权限。提权状态下拒绝 user scope，避免家目录与会话身份不一致。

计划列出归属检查、文件操作、带条件的命令、状态查询及详细 SCM 步骤。apply 重新生成并执行同样的动作。launchd bootstrap/bootout 根据 job 是否加载决定执行，已加载 job 必须来自预期的定义路径。保存的计划用于审阅，操作入口接收服务配置。

## 平台支持

| 后端 | Scope | 重启策略 | 实现与验证等级 |
| --- | --- | --- | --- |
| systemd / Linux | user、system | never、onFailure、always | 生命周期已实现；已链接构建；待真实生命周期验证 |
| launchd / macOS | user、system | never、onFailure、always | 生命周期已实现；已运行只读探测；待生命周期验证 |
| SCM / Windows | system | never、onFailure | 实验性 Host 与适配器；已编译检查；待本机验证 |
| OpenRC / Linux | system | never | 仅生成配置 |
| procd / Linux | system | never | 仅生成配置 |

systemd 使用用户或系统管理器，通过 target 链接启用自动服务。launchd 使用 `gui/UID` 或系统域；KeepAlive 会触发首次启动，因此拒绝 manual 与重启策略组合，以及带重启策略的 stop 操作。OpenRC 当前支持参数，尚不支持环境变量和工作目录。

## Windows Host

SCM 以 LocalSystem 运行随附的 Rust Host。每个服务的配置、Host 和日志位于 `C:\ProgramData\ServiceKit\ID`，ACL 仅授权 SYSTEM 与 Administrators。安装计划列出目录 ACL、文件放置、服务注册和恢复设置；安装步骤仅在受管理的 SCM 项不存在时执行。

Host 以挂起状态创建子进程，加入关闭即终止的 Job Object 后恢复执行。程序可通过 `SERVICE_KIT_STOP_EVENT` 命名事件协作停止；超时后 Host 终止进程树。SCM 负责失败重启，stdout、stderr 和 Host 错误分别追加到日志。

## 限制

定义文件通过原子写入和内容校验确认归属。相同配置重复安装、卸载已不存在的服务均幂等。变更服务时，先用原配置卸载，再安装新配置。

管理器失败可能保留已完成步骤，重试前应检查状态。事务更新、恶意并发路径变更、自定义 Windows 账号、日志轮转和其它 launchd 域仍需完善。生命周期、Windows ACL、Job Object 与恢复行为需要在对应平台的集成环境验证。

[English](../en/architecture.md) | [中文](architecture.md)
