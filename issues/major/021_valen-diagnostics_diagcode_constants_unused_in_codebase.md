---
scope: valen-diagnostics
severity: major
dimension: spec_coverage
---

# Diagcode Constants Unused In Codebase

## 概要

6 DiagCode constants defined but never emitted. JAVA_CALL_REQUIRES_SAFE critical for failure model.

## 現状

crates/valen-diagnostics/src/lib.rs:L88-L94

## 問題点

Java interop safety rules not enforced at compile-time.

## 改善案

Implement emission in relevant passes or mark as planned.

## 影響範囲

- crates/valen-diagnostics/src/lib.rs

## 関連ファイル

- crates/valen-hir/src/ty.rs
