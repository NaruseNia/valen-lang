---
scope: valen-codegen
severity: major
dimension: design
---

# class_emit.rs duplicates emit.rs functionality

## 概要
class_emit.rs が JVM IR を bypass して直接 ClassFile を構築。emit.rs と機能重複。

## 改善案
class_emit.rs を削除（emit.rs が全ケースを処理）か deprecated マーク。

## 影響範囲
- crates/valen-codegen/
