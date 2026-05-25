---
scope: valen-ast
severity: major
dimension: test_coverage
---

# Test Coverage Only Span Module

## 概要

Only 7 tests for Span. Zero tests for token.rs, Literal::span(), Expr::span(), Pattern::span(), Type::span().

## 現状

crates/valen-ast/tests/ast_tests.rs

## 問題点

Core public APIs completely untested.

## 改善案

Add tests for each span() method variant.

## 影響範囲

- crates/valen-ast/tests/ast_tests.rs

## 関連ファイル

(none)
