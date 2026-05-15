---
scope: valen-hir
severity: major
dimension: correctness
---

# JVM array type descriptor loses element type info

## 概要
classpath.rs parse_type_from_chars の '[' ケースで要素型を消費後に破棄。[Ljava/lang/String; が Array（型パラメータなし）になる。

## 改善案
TyRef::Generic("Array", vec![elem]) を返して要素型を保持。

## 影響範囲
- crates/valen-hir/
