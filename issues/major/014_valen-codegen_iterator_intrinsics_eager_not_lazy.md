---
scope: valen-codegen
severity: major
dimension: design
---

# Iterator Intrinsics Eager Not Lazy

## 概要

map()/filter() eagerly collect into ArrayList instead of lazy iterator wrappers.

## 現状

crates/valen-codegen/src/expr.rs:L2103

## 問題点

O(n) extra memory per transformation, infinite iterators impossible.

## 改善案

Implement lazy iterator wrapper classes.

## 影響範囲

- crates/valen-codegen/src/expr.rs

## 関連ファイル

(none)
