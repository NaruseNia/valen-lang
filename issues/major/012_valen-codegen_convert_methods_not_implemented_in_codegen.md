---
scope: valen-codegen
severity: major
dimension: spec_coverage
---

# Convert Methods Not Implemented In Codegen

## 概要

toLong(), toDouble() etc. not intrinsically handled. Would emit invalid invokevirtual on primitives.

## 現状

crates/valen-codegen/src/expr.rs

## 問題点

Numeric conversion methods unimplemented. User-reported.

## 改善案

Add intrinsic handling to emit JvmOp::Convert instead of InvokeVirtual.

## 影響範囲

- crates/valen-codegen/src/expr.rs

## 関連ファイル

(none)
