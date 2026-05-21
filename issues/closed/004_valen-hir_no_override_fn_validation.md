---
scope: valen-hir
severity: critical
dimension: spec_coverage
---

# No Override Fn Validation

## 概要

No validation of override fn requirement or open fn requirement for method overriding.

## 現状

crates/valen-hir/src/resolve.rs, ty.rs

## 問題点

Methods silently shadow parent methods without override keyword.

## 改善案

Add validation checking override/open requirements.

## 影響範囲

- crates/valen-hir/src/resolve.rs

## 関連ファイル

- docs/lang/05-classes.md
