---
scope: valen-parser
severity: minor
dimension: naming
---

# turbofish flag name is misleading

## 概要
PathSegment の `turbofish` フィールドが実際には `::` パス区切りを示すだけで、本来の turbofish (`::<T>`) 構文ではない。

## 改善案
`has_double_colon` にリネームするか、doc comment で実際のセマンティクスを明記。

## 影響範囲
- crates/valen-parser/src/parser.rs:L903-L905
- crates/valen-ast/src/lib.rs:L324-L327
