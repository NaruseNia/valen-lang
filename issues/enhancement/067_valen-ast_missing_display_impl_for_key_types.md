---
scope: valen-ast
severity: enhancement
dimension: idiomatic_rust
---

# Missing Display Impl For Key Types

## 概要

Only Span has Display. TokenKind, BinaryOp, etc. lack Display, degrading error messages.

## 現状

crates/valen-ast/src/token.rs

## 問題点

Error messages use Debug format.

## 改善案

Add Display for TokenKind, BinaryOp, UnaryOp, Visibility.

## 影響範囲

- crates/valen-ast/src/token.rs

## 関連ファイル

(none)
