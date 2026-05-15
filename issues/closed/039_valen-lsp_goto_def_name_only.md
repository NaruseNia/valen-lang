---
scope: valen-lsp
severity: major
dimension: correctness
---

# Goto def resolves by name only

## 概要
find_definition_at が HIR defs を名前文字列で線形スキャン。同名のローカル変数や異なるスコープの定義を区別しない。

## 改善案
カーソル位置を囲むスコープの定義を優先。HIR のスコープ情報を活用。

## 影響範囲
- crates/valen-lsp/
