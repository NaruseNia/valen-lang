---
scope: valen-ast
severity: minor
dimension: edge_case
---

# ソースオフセットが u32::MAX 超でラップアラウンド

## 概要

Span と LineIndex がバイトオフセットを u32 で保持。lexer/LSP/CLI で usize→u32 キャスト。4GB超のソースファイルでオフセットがラップし、不正な diagnostic 位置やスライスパニックが発生。

## 改善案

usize/u64 に変更するか、lexing/line-index 構築前に u32::MAX 超のファイルを明示的に reject + diagnostic。

## 影響範囲

- crates/valen-ast/src/span.rs
- crates/valen-parser/src/lexer.rs
- crates/valen-lsp/src/convert.rs
- crates/valenc/src/main.rs
