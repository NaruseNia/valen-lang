---
scope: valen-lsp
severity: major
dimension: correctness
---

# semantic_tokens_full uses byte length not UTF-16

## 概要
SemanticToken length が span.end - span.start（バイト長）。LSP は UTF-16 コードユニット長を期待。非 ASCII でハイライト範囲がずれる。

## 改善案
source テキストから token span を slice し encode_utf16().count() で長さ計算。

## 影響範囲
- crates/valen-lsp/
