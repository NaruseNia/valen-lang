---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# Any Type Not In Prelude

## 概要

Any type handled specially in is_subtype but not defined in prelude. Users get undeclared type error.

## 現状

crates/valen-hir/src/ty.rs:L3795

## 問題点

let x: Any = 42 fails with undeclared type.

## 改善案

Add Any to prelude as PrimTy or in core.vln.

## 影響範囲

- crates/valen-hir/src/ty.rs

## 関連ファイル

- docs/lang/02-types.md
