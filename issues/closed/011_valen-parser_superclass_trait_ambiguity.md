---
scope: valen-parser
severity: major
dimension: correctness
---

# Superclass vs trait ambiguity in class declaration

## 概要

parse_superclass_and_traits は `:` 後の最初の型を常に superclass として扱う。`class Foo : TraitA, TraitB` で TraitA が superclass に誤分類される。

## 改善案

フラット Vec<Type> に格納し、HIR 名前解決で分類を遅延させる。

## 影響範囲

- crates/valen-parser/src/parser.rs:L334-L344
