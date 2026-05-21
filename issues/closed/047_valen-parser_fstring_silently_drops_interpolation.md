---
scope: valen-parser
severity: major
dimension: correctness
---

# Fstring Silently Drops Interpolation

## 概要

If reparsed f-string expression has no tail, interpolation silently dropped with no diagnostic.

## 現状

crates/valen-parser/src/parser.rs:L2360-L2367

## 問題点

Silent data loss in output string.

## 改善案

Add fallback diagnostic when no tail expression found.

## 影響範囲

- crates/valen-parser/src/parser.rs:L2360-L2367

## 関連ファイル

(none)
