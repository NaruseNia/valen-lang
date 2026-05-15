---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# Vis::Internal treated identically to Vis::Pub

## 概要
lib.rs check_visibility で Vis::Internal => true。仕様は internal を同一 module 内限定と定義。

## 改善案
TODO コメントあり。module 境界実装時に実際のチェック追加。最低限 warning 発行を検討。

## 影響範囲
- crates/valen-hir/
