---
scope: valen-codegen
severity: major
dimension: correctness
---

# Comparison </<=/>/> on Object types uses reference equality

## 概要

Lt/Le/Gt/Ge が Object/Array 型で IfACmpNe にフォールスルー。参照比較は順序比較として無意味。

## 改善案

非 Comparable 型の順序比較を拒否するか compareTo() を呼び出す。

## 影響範囲

- crates/valen-codegen/src/expr.rs:L426-L432
