---
scope: valen-hir
severity: minor
dimension: correctness
---

# String concatenation does not check RHS type

## 概要
String + 演算子が LHS のみ String チェック。`"hello" + 42` が暗黙変換なしルールに反してエラーにならない。

## 改善案
RHS も String であることを要求。

## 影響範囲
- crates/valen-hir/src/ty.rs:L492-L495
