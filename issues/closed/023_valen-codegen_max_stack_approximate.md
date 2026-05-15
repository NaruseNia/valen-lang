---
scope: valen-codegen
severity: major
dimension: correctness
---

# max_stack calculation is approximate

## 概要
emit.rs の線形 stack-delta 累積が分岐制御フロー（if/else, ループ）を正しく扱わない。Frame リセット後に深い分岐のスタック深度を見逃す可能性。

## 改善案
基本ブロック単位の max_stack 解析を実装。

## 影響範囲
- crates/valen-codegen/
