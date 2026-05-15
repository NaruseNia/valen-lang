---
scope: valen-lsp
severity: major
dimension: correctness
---

# FileId hardcoded to 0 for all documents

## 概要
analyze_document が常に FileId(0) を使用。全ドキュメントの Span が同一 file id。

## 改善案
HashMap<Url, FileId> でドキュメント毎に一意 FileId を割当。

## 影響範囲
- crates/valen-lsp/
