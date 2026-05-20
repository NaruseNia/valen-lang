---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# Hover Lacks Rich Variable Info

## 概要

Hover on variables shows bare type only. Known issue #3.

## 現状

crates/valen-lsp/src/server.rs:L912-L970

## 問題点

No declaration context, doc comments, or enclosing function.

## 改善案

Show full let statement and enclosing function context.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
