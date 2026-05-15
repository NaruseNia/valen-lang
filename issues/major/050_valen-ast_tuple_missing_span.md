---
scope: valen-ast
severity: major
dimension: design
---

# Type::Tuple missing Span field

## 概要
Type::Tuple(Vec<Type>) に Span なし。空 Tuple で Span::DUMMY を返す回避策使用。他 Type variant は全て Span 保持。

## 改善案
Type::Tuple(Vec<Type>, Span) に変更。

## 影響範囲
- crates/valen-ast/
