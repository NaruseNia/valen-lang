---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# DataClassDecl missing supertypes field

## 概要
DataClassDecl に supertypes フィールドなし。ClassDecl にはある非対称。data class が trait を実装する宣言構文表現が不在。

## 改善案
DataClassDecl に supertypes: Vec<Type> を追加。

## 影響範囲
- crates/valen-ast/
