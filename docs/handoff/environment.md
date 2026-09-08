# 新电脑环境配置

本页记录可复现的安装选择，不表示新电脑已完成安装；不要整份复制旧电脑的用户配置或凭据。

## Git 身份

在新 clone 的仓库根目录运行：

```sh
git config --local user.name ttbb
git config --local user.email apples398@163.com
git var GIT_AUTHOR_IDENT
git var GIT_COMMITTER_IDENT
```

作者和提交者必须均为 `ttbb <apples398@163.com>`。环境变量可能覆盖上述配置；如不一致先纠正。
每次提交仍执行 `AGENTS.md` 中的前后检查和 Lore 提交格式。

## 游戏与 Python 测试

- 按根目录 README 和 CI 配置安装 Rust；保留 `Cargo.lock`，先运行 `cargo check --locked`。
- Python 纯测试没有额外依赖：

```sh
python3 -B -m unittest discover -s tools -p 'test_*.py' -q
python3 -B -m unittest discover -s tools/blender -p 'test_*.py' -q
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

依赖首次下载、系统图形库和完整游戏试玩不属于白模交接包完整性测试。

## Blender MCP（已验证组合，而非“最新版本”）

旧机器验证组合：Blender **5.2.1 LTS**、`blender-mcp==1.9.1`、ARM64 Python **3.11**。
在目标机器安装 Blender、uv、Codex，以及与机器架构一致的 Python。

Apple Silicon 上使用 Homebrew Python 的示例，先确认路径确实存在且架构正确：

```sh
/opt/homebrew/bin/python3.11 -c 'import platform; assert platform.machine() == "arm64"'
/opt/homebrew/bin/uvx --python /opt/homebrew/bin/python3.11 --no-build \
  --from blender-mcp==1.9.1 blender-mcp install-addon
codex mcp add blender --env DISABLE_TELEMETRY=true --env BLENDER_HOST=127.0.0.1 \
  -- /opt/homebrew/bin/uvx --python /opt/homebrew/bin/python3.11 --no-build \
  --from blender-mcp==1.9.1 blender-mcp
```

若已有同名 MCP，先检查配置，勿重复添加或覆盖其他服务。其他系统替换为实际的 uvx / Python 绝对路径；
`--no-build` 要求依赖有兼容的预编译包，缺包时应核对平台和 Python，而不是退回混合架构编译。
旧机器已卸载 Intel Python 3.12，不迁移该解释器、uv 缓存或其虚拟环境。

在 Blender 的插件偏好设置中启用 MCP for Blender，关闭 Allow Telemetry；
服务端环境变量也保留 `DISABLE_TELEMETRY=true`。连接仅使用本机回环地址，不开放公网端口。
重载客户端配置后先查询 addon status，再读取场景；不要以工具已列出替代实际连接测试。
插件支持执行 Python，仅让可信客户端使用，不恢复未知来源的脚本或自动运行状态。

## Codex / OMX 与技能

仓库 `AGENTS.md`、交接文档、历史计划可以迁移；用户级配置、技能、插件与聊天历史不在这个包里。
OMX 如需使用应在新机器单独安装/验证，不必把旧 `.omx/state` 搬过去。
`omx explore` 在本次整理环境的 PATH 中不可用，因此本次用直接 Git/文件检查验证；
不要因存在 `.omx` 目录就假定 OMX CLI 可运行。

用下面这段任务描述启动新会话即可：

> 请先阅读 AGENTS.md 和 docs/handoff/README.md，再检查当前 Git 状态、运行记录的测试。
> 当前完整可移植白模是 v08，M1 未验收；不要将 v09 纯几何预检当成 Blender 实建通过。
> 在不覆盖旧成果、不分发参考原图的前提下继续交接文档列出的下一步。
