# 08: 工具调用的详情里能读到用法说明（per-call · 通道三）

Type: implement
Status: ready-for-agent
Blocked by: 07, 04, 05
Part of: ../map.md

> 规格：[`.scratch/tool-declaration-visibility/map.md`](../map.md) 的**目的地**；形态由
> [04](04-grilling-visible-shape.md) 拍，内容（哪张表、哪个时刻的那张）由
> [05](05-grilling-which-table.md) 拍。
> **本票存在的前提**：04 拍的是「补 `description` 到 Schema 面」这一档（或它与 09 的组合）。
> 若 04 排除了这一档，本票随之关闭 —— **不实现**。

## 目标

- 工具调用详情覆盖层的 **`Schema` 面**先画**用法说明**、再画参数 schema —— 于是
  「**这次调用的那个工具声明是什么**」在界面上读得到（今天那个面只有参数 schema，`description`
  一个字都读不到）。
- 不新增入口、不新增事件：那个面已经在那里，只是少画了一段。

## 现状（开工前先复核）

- 那个面是 `FaceId::Schema`（[`src/render/tui.rs:14500-14502`](../../../src/render/tui.rs)）——
  工具那一行的**第三面**（六面：参数 / 输出 / Schema / 计时 / 来源 / 概述）。
- 它画的是 `parameters` 那一份，`pretty` 打印后按正文宽折行，与参数面同一套排法
  （[`src/render/tui.rs:14996-15009`](../../../src/render/tui.rs)）。
- 查不到时写一句「工具表里没有「<名字>」，这次调用的声明查不到」
  （[`src/render/wording.rs:549`](../../../src/render/wording.rs)），**不画空面、不编一份 schema
  出来** —— 这一条照旧。
- 用法说明最长的一条 896 字符（`ask_user_question`），最短的 63 字符（`write_file`）
  （研究 01 第 2 节）—— **两种长度都要读得下去**。
- 数据来自[票 07](07-account-carries-the-whole-declaration.md)（账带上整份声明）。

## 具体行为

1. 那个面的顺序：**用法说明在前，参数 schema 在后**；用法说明按正文宽折行（与参数面同一套排法）。
2. 长声明（896 字符）与短声明（63 字符）两种长度下都读得下去 —— 具体形状照
   [03 的帧](../prototype/03-shapes.txt)（若 03 已过帧）。
3. 「今天这张表」这句话按 05 的结论说（在哪儿说由 05 定）；查不到时**仍是那一句**，不新造措辞。
4. 不改那个面的**名字**与位置（`Schema` 面已经在六面分派表里）。

## 验收

- [ ] 点开一次工具调用，切到 `Schema` 面：**先**读到用法说明，**后**才是参数 schema。
- [ ] 最长那条（`ask_user_question`）与最短那条都读得下去。
- [ ] 查不到的工具仍是那一句「工具表里没有「<名字>」…」，**不画空面**。
- [ ] 模型可见文本零变化；事件流零变化。
- [ ] `cargo test` 全绿；`cargo clippy --all-targets` 无警告；`cargo fmt --check` 干净。
- [ ] 逐面文档与真机走查条目跟上（并进[票 10](10-docs-glossary-and-walkthrough.md)）。

## 评论

（空 —— 开工时补：落地与实测）
