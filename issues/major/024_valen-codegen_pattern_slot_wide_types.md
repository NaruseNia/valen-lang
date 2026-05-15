---
scope: valen-codegen
severity: major
dimension: correctness
---

# Pattern struct slot allocation ignores wide types

## 概要
expr.rs の Pattern::Struct 処理で self.next_slot += 1 がフィールドの wide 型（Long/Double = 2スロット）を考慮しない。

## 改善案
self.next_slot += field_ty.slot_count() に変更。

## 影響範囲
- crates/valen-codegen/
