---
scope: valenfmt
severity: major
dimension: test_coverage
---

# No Tests For M14 Expressions

## 概要

No tests for unsafe, cast, deref, ref mut, pipeline, list/map literal formatting.

## 現状

crates/valenfmt/tests/format_tests.rs

## 問題点

Only idempotency test provides weak coverage.

## 改善案

Add dedicated assert_format tests for each expression.

## 影響範囲

- crates/valenfmt/tests/format_tests.rs

## 関連ファイル

(none)
