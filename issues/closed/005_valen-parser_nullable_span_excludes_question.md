---
scope: valen-parser
severity: minor
dimension: correctness
---

# Nullable type T? span excludes question mark

## 概要
`String?` の type_span が `String` の span のみ返し、`?` を含まない。エラーハイライトが1文字ずれる。

## 改善案
Nullable ラッパーに `?` を含む span を格納。

## 影響範囲
- crates/valen-parser/src/parser.rs:L1439
