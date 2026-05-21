---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Generic Type Params In Path Expr Not Supported

## 概要

Path expression parser doesn't support generic type args. Vec::<Int>::new() cannot be parsed.

## 現状

crates/valen-parser/src/parser.rs:L1639-L1665

## 問題点

PathSegment.generics always empty in expressions.

## 改善案

Document as known limitation or implement turbofish.

## 影響範囲

- crates/valen-parser/src/parser.rs:L1639-L1665

## 関連ファイル

(none)
