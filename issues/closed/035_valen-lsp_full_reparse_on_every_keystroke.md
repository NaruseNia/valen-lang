---
scope: valen-lsp
severity: major
dimension: performance
---

# Full Reparse On Every Keystroke

## 概要

Every didChange triggers full re-analysis of changed doc + ALL other open docs.

## 現状

crates/valen-lsp/src/server.rs:L99-L135

## 問題点

O(N) full re-analyses per keystroke.

## 改善案

Add debounce, incremental parsing, dependency-based re-analysis.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
