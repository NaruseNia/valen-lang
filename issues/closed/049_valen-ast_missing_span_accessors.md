---
scope: valen-ast
severity: major
dimension: design
---

# Missing span() accessor methods causing code duplication

## 概要
Literal/Expr/Type/Pattern に span() メソッドなし。parser と valenfmt に literal_span/expr_span が重複。

## 改善案
valen-ast に各型の span() メソッドを追加、消費側を移行。

## 影響範囲
- crates/valen-ast/
