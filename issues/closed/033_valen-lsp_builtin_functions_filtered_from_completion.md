---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# Builtin Functions Filtered From Completion

## 概要

Prelude functions (println, print) explicitly skipped in completion. Known issue #14.

## 現状

crates/valen-lsp/src/server.rs:L755

## 問題点

Users can't discover built-in functions.

## 改善案

Include prelude functions with lower sort priority.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
