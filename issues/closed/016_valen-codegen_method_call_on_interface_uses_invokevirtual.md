---
scope: valen-codegen
severity: major
dimension: correctness
---

# Method Call On Interface Uses Invokevirtual

## 概要

Trait method calls emit InvokeVirtual instead of InvokeInterface. Causes IncompatibleClassChangeError.

## 現状

crates/valen-codegen/src/expr.rs:L411-L429

## 問題点

Runtime error when calling trait methods.

## 改善案

Check if receiver is interface and emit InvokeInterface.

## 影響範囲

- crates/valen-codegen/src/expr.rs:L411-L429

## 関連ファイル

(none)
