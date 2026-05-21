---
scope: valen-codegen
severity: major
dimension: error_handling
---

# Integer Literal Panic On Overflow

## 概要

Integer literal overflowing i32 panics instead of returning proper error.

## 現状

crates/valen-codegen/src/expr.rs:L355-L356

## 問題点

Panic in compiler instead of diagnostic.

## 改善案

Return CodegenError instead of panicking.

## 影響範囲

- crates/valen-codegen/src/expr.rs:L355-L356

## 関連ファイル

(none)
