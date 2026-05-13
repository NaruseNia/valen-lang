---
scope: valen-codegen
severity: major
dimension: spec_coverage
---

# Record variant fields have no accessor methods

## 概要

record variant フィールドが private final だがアクセサメソッドが生成されない。Java record は各コンポーネントの public アクセサが必要。

## 改善案

Java record 規約に従い public アクセサメソッドを生成。

## 影響範囲

- crates/valen-codegen/src/lower.rs:L350-L388
