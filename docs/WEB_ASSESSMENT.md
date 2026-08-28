# B-26：Web（WebGPU / WASM）移植评估报告

> 状态：评估完成（2026-08）· 决策待人工（HITL）· 关联 BACKLOG B-26、PRD §4.7
> 参考：[Bevy 0.19 WebGPU/wasm 游戏案例](https://github.com/xiongchenyu6/protect-carrot)、[Bevy 0.13 wgpu 升级说明](https://raw.githubusercontent.com/bevyengine/bevy-website/85e8a359fb7f0146e975f21e65c2b657894a5821/content/news/2024-02-17-bevy-0.13/index.md)

## 一、结论摘要

**Web 移植可行，建议分两期，一期（最小可玩）约 2-4 周。** 主要障碍不是渲染（Bevy 0.19 的 WebGPU 后端已有成功案例），而是**文件系统层**（存档/进度/截图依赖 `std::fs`）与**平台差异适配**。

## 二、现状调研（2026-08）

### 2.1 引擎侧 ✅
- Bevy 0.19 支持 `wasm32-unknown-unknown` 目标 + WebGPU 后端（wgpu 29）
- 已有 **Bevy 0.19 WebGPU/wasm 实际游戏**（保卫萝卜塔防）验证可行
- WebGL2 后备（旧浏览器），WebGPU 主推（Chrome/Edge 113+）

### 2.2 本项目依赖逐项分析

| 依赖 | Web 支持 | 备注 |
|------|----------|------|
| bevy 0.19（渲染/UI/音频/输入） | ✅ | 官方 wasm 目标；WebGPU 后端 |
| bevy_egui 0.42 | ✅ | 官方提供 wasm demo |
| avian3d 0.7（物理） | ⚠️ 待验证 | 内核 parry3d 有 wasm 先例；需一次编译验证 |
| serde/ron/bincode | ✅ | 纯 Rust |
| WAV 音频（bevy_audio + wav 特性） | ✅ | 浏览器原生支持 wav |
| CJK 字体（otf 资产） | ✅ | 浏览器字体加载 |

### 2.3 关键平台差异（主要工作量）

| 能力 | 原生 | Web 现状 | 适配方案 |
|------|------|----------|----------|
| 存档/进度（`std::fs`） | ✅ saves/ 目录 | ❌ 无文件系统 | **存储抽象层**：trait `Storage { save/load }`，原生实现文件、Web 实现 **IndexedDB**（通过 wasm-bindgen 或 js 桥） |
| 截图导出 | ✅ 写 PNG 文件 | 部分 | 浏览器用 Blob + 下载链接；离屏渲染目标方案可复用 |
| 分享（.ptw 导入导出） | ✅ 文件 | ⚠️ | 下载/拖放文件；云端分享码是更自然路径 |
| PHOENIX_STRESS/LOAD 环境变量 | ✅ | ⚠️ | URL 参数或构建期特性替代 |
| 压力测试规模 | 50k 块 ~110FPS | 待测 | 目标降档（10k 块 ≥ 30FPS） |

### 2.4 性能预算
- Web 目标建议：**10,000 积木 ≥ 30 FPS**（桌面浏览器独显）；共享资产实例化架构已为低 draw call 打好基础
- 需在目标浏览器实测（原生 50k 基准不可直接外推）

## 三、分期建议

| 阶段 | 范围 | 预估 | 风险 |
|------|------|------|------|
| W1 最小可玩 | 渲染+轨道相机+放置+蓝图模式+教程；存储抽象（IndexedDB） | 2-4 周 | 中（存储层 + wasm 构建链） |
| W2 完整功能 | 物理/音频/挑战/图鉴/截图/分享（下载+拖放） | 4-6 周 | 低中（逐项适配） |
| W3 云端分享码 | 服务端 + 分享码 | 另行立项 | 中（需后端） |

## 四、决策点（需人工拍板）

1. **是否投入**：Web 版的目标用户/场景（教育课堂免安装？）是否支撑开发投入
2. **目标浏览器**：仅 WebGPU（Chrome/Edge 113+）还是需 WebGL2 回退（范围扩大）
3. **优先级**：W1 先行 vs 待 Phase 3 运营数据再定
4. **存储方案**：IndexedDB 直写 vs 通过 JS 桥（决定 wasm-bindgen 依赖范围）

## 五、推荐

**暂缓 Web 移植，优先完成原生版本的运营验证**（PRD 平台目标本就是 Desktop 优先）。
若教育场景获客需求明确，按 W1 范围启动，首个技术验证点：
`wasm32-unknown-unknown` 构建通过 + avian3d 编译 + 渲染冒烟 —— 一天内可出结论。
