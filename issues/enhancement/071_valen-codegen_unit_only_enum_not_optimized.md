---
scope: valen-codegen
severity: enhancement
dimension: performance
---

# Unit Only Enum Not Optimized

## 概要

Unit-only enums emit sealed interface + singletons instead of Java enum. User-reported.

## 現状

crates/valen-codegen/src/lower.rs:L978-L1095

## 問題点

4 class files instead of 1 for simple enums.

## 改善案

Detect all-unit enums and emit Java enum class.

## 影響範囲

- crates/valen-codegen/src/lower.rs

## 関連ファイル

(none)
