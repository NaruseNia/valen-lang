---
scope: valen-codegen
severity: major
dimension: correctness
---

# int literal i64 to i32 truncation

## 概要

IntLit 値が `*n as i32` で i32 範囲外の値を黙って切り捨て。

## 改善案

debug_assert で i32 範囲チェック、または i32::try_from を使用。

## 影響範囲

- crates/valen-codegen/src/expr.rs:L164
