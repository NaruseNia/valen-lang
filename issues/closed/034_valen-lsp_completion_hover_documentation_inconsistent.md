---
scope: valen-lsp
severity: major
dimension: design
---

# Completion Hover Documentation Inconsistent

## 概要

Completion and hover built by separate code paths with different output. Known issue #9.

## 現状

crates/valen-lsp/src/server.rs

## 問題点

~300 lines of duplicated formatting code producing different results.

## 改善案

Unify into single build_def_documentation() function.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
