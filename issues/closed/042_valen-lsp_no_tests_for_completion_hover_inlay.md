---
scope: valen-lsp
severity: major
dimension: test_coverage
---

# No Tests For Completion Hover Inlay

## 概要

Zero tests for completion, hover, semantic tokens, inlay hints. Only diagnostics/goto-def tested.

## 現状

crates/valen-lsp/tests/analysis_tests.rs

## 問題点

Majority of LSP functionality untested.

## 改善案

Add integration tests for each build_* function.

## 影響範囲

- crates/valen-lsp/tests/analysis_tests.rs

## 関連ファイル

(none)
