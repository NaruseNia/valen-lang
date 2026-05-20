---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# Int Literal Uses I64 Instead Of I32

## 概要

Literal::Int stores i64 but spec says Int is 32-bit JVM int. Allows silent overflow.

## 現状

crates/valen-ast/src/lib.rs:L456, token.rs:L12

## 問題点

3_000_000_000 stored as Int without range check.

## 改善案

Change to Int(i32, Span) or add validation.

## 影響範囲

- crates/valen-ast/src/lib.rs:L456

## 関連ファイル

(none)
