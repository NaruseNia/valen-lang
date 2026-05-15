---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Lexer missing Char literal

## 概要
Char リテラル（`'a'`）のトークン規則がlexerに存在しない。仕様は Char をプリミティブ型として定義、TokenKind::CharLit は AST に存在。

## 現状
crates/valen-parser/src/lexer.rs:L9: deferred として記載。parser の parse_literal() は TokenKind::CharLit を処理するが lexer が生成しない。

## 改善案
`#[regex(r"'([^'\\]|\\.)'")]` 等の char literal regex を追加。

## 影響範囲
- crates/valen-parser/src/lexer.rs
