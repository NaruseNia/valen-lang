---
scope: valen-lsp
severity: major
dimension: test_coverage
---

# No tests for completion/hover/semantic tokens

## 概要
テストが diagnostics と goto-def（HIR 存在確認のみ）のみ。completion, hover, semantic tokens は完全未テスト。

## 改善案
各コンテキスト（general, type, ::, dot, impl）の completion テスト、hover テスト、semantic tokens テストを追加。

## 影響範囲
- crates/valen-lsp/
