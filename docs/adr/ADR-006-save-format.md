# ADR-006：存档格式与兼容策略

- 状态：建议（Phase 1 落地时接受）
- 关联：docs/PRD.md §3.5/§4、docs/REVIEW.md P10、BACKLOG B-11

## 背景

方案 §4.6 定义存档：版本号 + 世界种子/模式 + 积木列表。需补充 schema 演进策略，防止旧档在版本升级后不可读（玩家数据底线）。

## 决策

1. **文件格式**：`.ptw` = bincode 压缩（`serde` + `bincode`），扩展名统一；同时支持 JSON 导出（分享/调试）。
2. **结构**：
   ```rust
   struct SaveFile {
       version: u32,            // 存档格式版本，必须
       game_mode: Mode,         // Blueprint / FreeBuild / Challenge
       seed: u64,               // 挑战随机种子（物理确定性回放，见 ADR-002）
       blocks: Vec<PlacedBlockRecord>,  // { id, cell: IVec3, rot_90: u8, props: Option<BlockProps> }
       meta: SaveMeta,          // 完成度快照、时间戳、校验和
   }
   ```
3. **兼容策略**：`version` 只增不减；读档按版本走迁移函数链（v1 → v2 → …），未知版本拒绝加载并提示。
4. 保存时机：每 30s 自动 + 显式保存；写入临时文件后原子重命名，防中断损坏。

## 后果

- 优点：玩家数据可跨版本；挑战模式可种子回放。
- 成本：每个积木类型变更（增删 BlockId）都视为格式变更，需走迁移链——积木 ID 一旦发布**不可复用**。
