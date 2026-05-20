---
scope: valen-codegen
severity: major
dimension: spec_coverage
---

# Break With Value Not Implemented

## 概要

break val silently discards the value expression. Loop expression values lost.

## 現状

crates/valen-codegen/src/expr.rs:L468-L471

## 問題点

Value from break expr silently dropped.

## 改善案

Lower value expression before Goto.

## 影響範囲

- crates/valen-codegen/src/expr.rs:L468-L471

## 関連ファイル

(none)
