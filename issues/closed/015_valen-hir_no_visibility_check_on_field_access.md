---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# No visibility check on field access

## 概要

resolve_field_type が ctor_params の可視性を確認しない。private フィールドがクラス外部からアクセス可能。

## 改善案

外部アクセス時に Vis::Pub または Vis::Internal を要求する可視性チェックを追加。

## 影響範囲

- crates/valen-hir/src/ty.rs:L925-L941
