---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# No Entry Point Validation

## 概要

No validation that fn main() exists or has correct signature. User-reported concern.

## 現状

crates/valen-hir/src/resolve.rs

## 問題点

Programs without fn main() compile without error.

## 改善案

Add validation pass for main function existence/signature.

## 影響範囲

- crates/valen-hir/src/resolve.rs

## 関連ファイル

- crates/valenc/src/main.rs
