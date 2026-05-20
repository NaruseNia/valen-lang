---
scope: valen-ast
severity: major
dimension: correctness
---

# Span Merge Panics In Production Code

## 概要

Span::merge uses assert_eq! which panics on file_id mismatch instead of returning an error gracefully.

## 現状

crates/valen-ast/src/span.rs:L40-L50

## 問題点

A parser bug or malformed input triggers this assert, crashing the compiler.

## 改善案

Use debug_assert_eq! or return Result<Span, SpanError>.

## 影響範囲

- crates/valen-ast/src/span.rs:L40-L50

## 関連ファイル

- crates/valen-parser/src/parser.rs
