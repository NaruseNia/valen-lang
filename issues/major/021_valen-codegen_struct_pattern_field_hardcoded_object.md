---
scope: valen-codegen
severity: major
dimension: correctness
---

# Struct pattern field type hardcoded to Object

## 概要

lower_pattern_check の Pattern::Struct で全フィールド型が `JvmType::Object("java/lang/Object")` にハードコード。GetField ディスクリプタが不正になる。

## 改善案

variant 定義から実際のフィールド型を解決する型マップを渡す。

## 影響範囲

- crates/valen-codegen/src/expr.rs:L631
