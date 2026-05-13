---
scope: valen-ast
severity: minor
dimension: design
---

# Pattern::Tuple and Pattern::Or missing Span

## 概要
他のバリアントと異なり Span を持たない。エラー報告でソース位置を指示不可。

## 改善案
`Tuple(Vec<Pattern>, Span)` と `Or(Vec<Pattern>, Span)` に変更。

## 影響範囲
- crates/valen-ast/src/lib.rs:L443-L445
