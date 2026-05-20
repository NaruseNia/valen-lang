---
scope: valen-hir
severity: major
dimension: correctness
---

# Field Access Ignores Generic Type Substitution

## 概要

resolve_field_type only handles Ty::Named, not Ty::Generic. Generic field types not substituted.

## 現状

crates/valen-hir/src/ty.rs:L2318-L2329

## 問題点

Box<Int>.value fails because receiver is Ty::Generic.

## 改善案

Match on both Named and Generic, build type param bindings.

## 影響範囲

- crates/valen-hir/src/ty.rs:L2318-L2329

## 関連ファイル

(none)
