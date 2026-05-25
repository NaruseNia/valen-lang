---
scope: valen-hir
severity: major
dimension: test_coverage
---

# No Test For Numeric Conversion

## 概要

No tests for toLong(), toFloat(), toDouble() resolution. Masks the inherent impl bug.

## 現状

crates/valen-hir/src/ty.rs tests

## 問題点

User-reported bug untested.

## 改善案

Add tests: 42.toLong(), 3.14f.toDouble().

## 影響範囲

- crates/valen-hir/src/ty.rs

## 関連ファイル

(none)
