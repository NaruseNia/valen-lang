---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# No Package Info In Completions

## 概要

No package information in completion or hover. Known issue #8.

## 現状

crates/valen-lsp/src/server.rs

## 問題点

Can't distinguish identically-named types from different packages.

## 改善案

Extract package from AST, include in completion detail.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
