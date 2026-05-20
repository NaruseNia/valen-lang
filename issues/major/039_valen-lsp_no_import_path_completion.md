---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# No Import Path Completion

## 概要

Import statements have no completion. Known issue #12.

## 現状

crates/valen-lsp/src/server.rs

## 問題点

No ImportContext variant.

## 改善案

Add import context detection and package path completion.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
