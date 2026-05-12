---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Missing assignment expression parsing

## 概要

AST に `Expr::Assign(AssignExpr)` が定義されているがパーサーは生成しない。`x = 10` や `count += 1` がパースできない。

## 改善案

parse_expr にアサインメントパーシングを追加。=, +=, -=, *=, /=, %= を処理。

## 影響範囲

- crates/valen-parser/src/parser.rs:L620-L622
