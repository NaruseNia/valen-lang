---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# Missing Hex And Binary Literal Token Kinds

## 概要

No support for 0xFF or 0b1010 literals. User-reported gap.

## 現状

crates/valen-ast/src/token.rs:L11-L19

## 問題点

Users cannot write hex/binary literals.

## 改善案

Fix belongs in lexer; token kind is fine if lexer parses 0x/0b prefixes into IntLit.

## 影響範囲

- crates/valen-ast/src/token.rs:L11-L19

## 関連ファイル

- crates/valen-parser/src/lexer.rs
