---
scope: valen-codegen
severity: major
dimension: spec_coverage
---

# Println Print Only Accepts String

## 概要

println/print always emit String descriptor. println(42) causes VerifyError at runtime.

## 現状

crates/valen-codegen/src/expr.rs:L2756-L2773

## 問題点

Type mismatch for non-String arguments.

## 改善案

Select PrintStream overload based on argument type.

## 影響範囲

- crates/valen-codegen/src/expr.rs:L2756-L2773

## 関連ファイル

(none)
