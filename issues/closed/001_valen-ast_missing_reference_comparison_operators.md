---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# Missing === and !== reference comparison operators

## 概要

仕様 §2.2 で MVP 演算子として定義された `===`/`!==` が BinaryOp にも TokenKind にも存在しない。

## 改善案

BinaryOp に RefEq/RefNe、TokenKind に対応するバリアントを追加し、lexer/parser に接続する。

## 影響範囲

- crates/valen-ast/src/lib.rs:L372-L391
- crates/valen-ast/src/token.rs:L82-L84
- crates/valen-parser/src/lexer.rs:L134-L137
