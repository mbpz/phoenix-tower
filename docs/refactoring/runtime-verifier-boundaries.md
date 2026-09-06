# 原生验证器边界跟进

基线：`22cc372`；2026-09-06。

## 修改前计划

- 范围仅 `tools/verify_runtime.py`、对应 Python 测试和风险文档，不改游戏、依赖或存档。
- 日志读取目前每次读 1 MiB，但无换行的 pending 会无限增长；退出后追读至 EOF 也没有总量边界。先补失败测试，再设单行和总解析字节预算；超限必须验收失败，不能截断后宣称成功。原始 stdout/stderr 仍直写 runtime.log。
- 首次 Ctrl-C 会进入 finally，但第二次 SIGINT 可打断回收。先补真实信号回归，再在 CLI 主线程的收尾阶段临时忽略 SIGINT，完成回收和有限日志读取后恢复原 handler。
- 保持进程所有权，不按名字杀进程，不写用户槽位。工具仍非不可信进程沙箱，磁盘配额、派生子进程及不可杀进程不在本轮保证内。
- 门禁：先观察新测试失败，再跑完整 Python 回归、Rust 回归及现有静态门禁；独立复核后记录剩余边界。

## 实施与测试

- 复用现有 `drain`，不引入解析器框架：64 KiB 单行预算（不含 LF）及 64 MiB 总读取预算；总量多读至多 1 字节确认超限。pending 最多保留一条合规行，每轮仍最多读取 1 MiB。
- 超限时停止解析、回收子进程并将报告标记 failed；即使已完成观测、仅在最后追读发现超限，也不能通过。未解析内容不再分类，不能以错误计数为 0 宣称日志无错误。原始文件未截断，摘要记录预算、已读字节和原始字节。
- CLI 主线程在 finally 内临时设 SIGINT 为忽略，完成回收和有限追读后恢复原处理器。没有全局永久禁用 Ctrl-C，也不修改游戏进程的处理器。
- 最初三项新回归在旧实现上失败：无换行超限和短行累计超限被误判通过；重复 SIGINT 逃逸清理。红灯日志：`/tmp/phoenix-boundaries-red.log`。
- 最终新增六项回归：无换行超限、带换行超限不能提供就绪证据、短行累计超限、精确预算下 UTF-8 字节保留、仅最后追读发现超限、真实 SIGINT 重复发送与 handler 恢复。测试使用自有 Python 子进程，并在断言前确认验证器已回收；测试自身也有兜底清理。
- Python **40 通过**；Rust **98 通过、1 ignored**；locked offline all-target check、严格 Clippy、fmt、diff 检查通过。未修改 Rust 或新增依赖。日志：`/tmp/phoenix-boundaries-{green,rust,check,clippy}.log`。
- 独立只读复核：无可操作问题，运行三个重点回归及 AST/diff 检查；LSP 不可用，不声称 Python 类型验证已完成。

## 本机原生验证

2026-09-06，Apple M1 / Metal，使用既有已构建游戏二进制和本轮验证器：
`python3 -B tools/verify_runtime.py --riverside --measure-fps --warmup 2 --duration 5 --timeout 60`。

通过：江岸 8 块可编辑样板、UI 正尺寸、5 个带生产时间戳的窗口内 FPS 样本，瞬时 FPS 中位数 60.0、最低 55.6。完整运行 11.71 秒，日志 105,053 字节，全部读取，未触及预算；910 条已知 ICU 警告原样保留，未识别错误 0，子进程已回收。此次短运行证明新预算未误伤普通样板，不是新的性能优化或长时负载结论。

原始摘要：`/private/tmp/phoenix-boundaries-native/20260906T004350Z-xz0l3ysd/summary.json`；持久证据：`runtime-verifier-boundaries-evidence.json`。

## 仍保留的边界

- 解析预算不是磁盘配额：stdout/stderr 直写文件，快速输出在轮询/终止宽限内仍可写入更多数据；磁盘满和恶意派生进程不受此工具隔离。
- 只管理直接启动的子进程。5 秒是正常终止宽限，不是包含 OS 强制回收、I/O 和解析的总时间硬上限。
- 信号回归在 Python 主线程向自己发送真实 SIGINT，结合真实测试子进程；未执行独立终端连续按键或 Windows 控制台信号验收。强制 SIGKILL、第三方异步异常、非主线程调用没有同等中断保证。
- 上游中文分词、50k 渲染性能、Windows/Linux GPU 及完整人工玩法风险不因本轮工具修复而消除。

## 文件

- `tools/verify_runtime.py`：有限解析和局部信号保护。
- `tools/test_verify_runtime.py`：六项边界测试与测试子进程兜底清理。
- `README.md`、`riverside-risk-followup.md`：使用限制和历史风险状态。
- 本记录与 `runtime-verifier-boundaries-evidence.json`：计划、验收和长期证据。
