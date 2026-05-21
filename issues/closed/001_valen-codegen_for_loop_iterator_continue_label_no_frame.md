---
scope: valen-codegen
severity: critical
dimension: correctness
---

# For Loop Iterator Continue Label No Frame

## 概要

continue_label in for-iterator has no StackMapTable frame. May cause VerifyError.

## 現状

crates/valen-codegen/src/expr.rs:L1824-L1827

## 問題点

Missing frame at continue target.

## 改善案

Merge continue_label with loop_label or add frame.

## 影響範囲

- crates/valen-codegen/src/expr.rs:L1824-L1827

## 関連ファイル

(none)
