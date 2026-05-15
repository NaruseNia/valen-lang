---
scope: valen-lsp
severity: major
dimension: test_coverage
---

# Goto def tests only verify HIR name, not position

## 概要
テストが hir.defs で名前検索するのみ。find_definition_at の実際の呼び出しや Location range 検証なし。

## 改善案
ServerState を構築し特定位置でクエリ、返却 Location の URI/range を検証。

## 影響範囲
- crates/valen-lsp/
