---
scope: valen-diagnostics
severity: major
dimension: design
---

# Labels And Notes Never Populated

## 概要

Diagnostic.labels and .notes fields exist but never populated. No builder API to use them.

## 現状

crates/valen-diagnostics/src/lib.rs:L35-L36

## 問題点

Rich diagnostics infrastructure unused.

## 改善案

Add with_label/with_note builder methods. Update LSP to emit relatedInformation.

## 影響範囲

- crates/valen-diagnostics/src/lib.rs

## 関連ファイル

- crates/valen-lsp/src/convert.rs
