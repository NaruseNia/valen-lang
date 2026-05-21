---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# Override Keyword Missing From Completion

## 概要

override not in EXPR_KEYWORDS. Known issue #15.

## 現状

crates/valen-lsp/src/server.rs:L2979-L2983

## 問題点

override keyword not offered as completion.

## 改善案

Add override and other missing keywords.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
