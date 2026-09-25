# 使用指南

## 快速开始

使用 `cargo build --release -p service-kit-cli` 构建 CLI；开发环境见[贡献指南](contributing.md)。先探测管理器并查看安装计划：

```sh
mise exec -- cargo run -p service-kit-cli -- probe --scope user
mise exec -- cargo run -p service-kit-cli -- plan install --spec examples/service.json --backend launchd
```

可显式指定 `systemd` 离线生成 Linux 配置。Windows 使用盘符绝对路径、system scope 和 `scm` 后端。显式后端的计划可以跨宿主生成；自动选择需要当前主机上可达且具备权限的后端。

真实安装前，将示例改为实际长期运行的程序。`install --spec FILE --backend NAME --dry-run` 返回计划；去掉 `--dry-run` 才执行。system scope 的修改操作需要提权，状态查询只申请读权限。安装通常只注册或启用；launchd 的 RunAtLoad/KeepAlive 可能在加载时启动程序。

状态和卸载都使用原有配置。首版拒绝覆盖不同内容，暂不提供原地更新；修改配置前先用旧配置卸载。

## 服务配置

`--spec FILE` 通过 Figment provider 读取 TOML（`.toml`）、JSON（`.json`）和 YAML（`.yaml`/`.yml`），按扩展名选择格式，不区分大小写。各格式共用字段和校验规则，未知扩展名、格式错误和文件缺失均明确报错。不隐式读取环境变量覆盖或搜索配置文件；CLI 响应仍为 JSON。

```json
{
  "id": "example.worker",
  "executable": "/opt/example/worker",
  "args": ["--message", "literal $HOME; no shell"],
  "environment": { "MODE": "production" },
  "workingDirectory": "/opt/example",
  "description": "Example worker",
  "scope": "user",
  "startup": "automatic",
  "restart": "onFailure",
  "backendOptions": { "systemd": { "afterNetwork": true } }
}
```

`id` 为 1–128 位 ASCII 字母、数字、点、下划线或连字符，以字母数字开头。可执行文件及工作目录必须为所选后端平台的绝对路径，不允许父级（`..`）路径分量或控制字符。Windows 首期不支持 UNC 路径或 user scope。

`scope` 为 `user|system`，`startup` 为 `manual|automatic`，`restart` 为 `never|onFailure|always`，三者均需明确提供。automatic 对 user/system scope 分别对应登录/系统启动语义，受后端能力限制。args、environment、description、backendOptions 默认空，workingDirectory 可省略；未知字段报错。

环境变量名遵循 `[A-Za-z_][A-Za-z0-9_]*`。参数、环境值和说明不允许控制字符。Windows 拒绝大小写重复的变量名及保留变量 `SERVICE_KIT_STOP_EVENT`。

后端选项：systemd 的 `afterNetwork`；launchd 的 `processType`（Background/Standard/Interactive/Adaptive）；scm 的 `stopTimeoutSecs`（1–300，默认 20）。其它后端的选项不会被静默忽略。

OpenRC 暂不支持 environment/cwd；OpenRC/procd 仅接受 restart=never；launchd manual 仅接受 restart=never；SCM 拒绝 restart=always。配置和计划可能包含环境机密，应控制文件访问。

## CLI 与 JSON 协议 v1

```text
service-kit probe [--scope user|system]
service-kit plan install|uninstall|start|stop|restart|status --spec FILE [--backend auto|NAME] [--priority systemd,launchd,scm]
service-kit install|uninstall|start|stop|restart|status --spec FILE [--backend auto|NAME] [--priority systemd,launchd,scm] [--dry-run]
```

常规操作向 stdout 输出一个 JSON 响应，诊断写 stderr；失败退出码非零。帮助和版本保持文本形式。成功 probe 可包含被排除的候选，需检查每个 candidate。

```json
{"protocolVersion":1,"success":true,"data":{"backend":"systemd"},"error":null}
```

```json
{"protocolVersion":1,"success":false,"data":null,"error":{"code":"UNSUPPORTED","message":"scope/restart unsupported"}}
```

稳定错误码：`INVALID_SPEC`、`UNSUPPORTED`、`UNAVAILABLE`、`PERMISSION_DENIED`、`CONFLICT`、`IO`、`COMMAND_FAILED`、`NOT_FOUND`、`INVALID_ARGUMENTS`、`INVALID_HOST`、`UNSAFE_PATH`、`MANAGER_FAILED`、`TIMEOUT`。状态为 running/stopped/failed/notInstalled/unknown。通过 state 字段判断服务运行状态。

计划包含 backend、operation、serviceId、scope、actions、warnings，字段统一为 camelCase；动作包含定义检查、文件操作、带条件的命令、状态查询和 SCM 步骤。apply 从配置重新生成并执行这些步骤。

```ts
import { ServiceKit, Scope, Operation, BuiltinBackend } from "./sdk/node/index.ts";
const client = new ServiceKit({ binary: "/absolute/path/service-kit" });
const probes = await client.probe(Scope.User);
const plan = await client.plan(Operation.Install, "./service.toml", { backend: BuiltinBackend.Launchd });
```

SDK 按显式 binary、`SERVICE_KIT_BINARY`、SDK 本地 bin、PATH 查找 CLI；无 shell 执行、限制超时和输出量、校验协议版本。传输错误为 TRANSPORT_ERROR，找不到程序为 BINARY_NOT_FOUND，非法响应为 INVALID_PROTOCOL，版本不兼容为 UNSUPPORTED_PROTOCOL_VERSION。`manageWithSelector` 读取探测结果后调用用户选择函数，再把明确后端名传回 Rust。

[English](../en/guide.md) | [中文](guide.md)

