---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# Missing Char variant in AST Literal enum

## 概要

TokenKind に CharLit(char) があるが AST Literal に Char バリアントがなく、char リテラルを AST で表現不可。

## 改善案

`Char(char, Span)` を Literal enum に追加。

## 影響範囲

- crates/valen-ast/src/lib.rs:L308-L314
