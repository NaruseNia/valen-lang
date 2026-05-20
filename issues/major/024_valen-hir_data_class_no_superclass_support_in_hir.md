---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# Data Class No Superclass Support In Hir

## 概要

DataClassDef has no superclass field. Data classes can't express sealed class inheritance in HIR.

## 現状

crates/valen-hir/src/lib.rs:L291-L297

## 問題点

Spec allows data class as sealed permit leaf, HIR can't represent it.

## 改善案

Add superclass field to DataClassDef.

## 影響範囲

- crates/valen-hir/src/lib.rs:L291-L297

## 関連ファイル

- docs/lang/05-classes.md
