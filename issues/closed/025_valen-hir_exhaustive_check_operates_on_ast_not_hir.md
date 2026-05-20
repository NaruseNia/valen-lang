---
scope: valen-hir
severity: major
dimension: design
---

# Exhaustive Check Operates On Ast Not Hir

## 概要

Exhaustiveness checker takes raw AST and re-infers types instead of using typed HIR. Fragile and inaccurate.

## 現状

crates/valen-hir/src/exhaustive.rs:L16-L26

## 問題点

Many match expressions silently skipped due to failed type inference.

## 改善案

Refactor to operate on TypedBody/TypedExpr.

## 影響範囲

- crates/valen-hir/src/exhaustive.rs

## 関連ファイル

- crates/valen-hir/src/ty.rs
