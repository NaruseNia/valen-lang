---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Lexer missing === and !== operators

## 概要
参照等値演算子 `===` / `!==` のトークン規則がlexerにない。仕様 (02-types.md) で MVP 利用可能と定義。

## 現状
RawTok に EqEqEq/NotEqEq variant なし。`a === b` は EqEq + Eq + Ident(b) としてlexされる。TokenKind::EqEqEq/NotEqEq は valen-ast に存在。

## 改善案
`#[token("===")] EqEqEq` と `#[token("!==")] NotEqEq` を EqEq/NotEq より前に追加。parse_eq() で BinaryOp::RefEq/RefNe として処理。

## 影響範囲
- crates/valen-parser/src/lexer.rs
- crates/valen-parser/src/parser.rs
