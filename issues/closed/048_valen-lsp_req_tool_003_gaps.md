---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# REQ-TOOL-003 acceptance criteria gaps

## 概要
goto-def がスコープ非対応、cross-file 型チェック不能、ローカル変数の goto-def 不能。

## 改善案
各ギャップを known limitation として文書化し段階的に対応。

## 影響範囲
- crates/valen-lsp/
