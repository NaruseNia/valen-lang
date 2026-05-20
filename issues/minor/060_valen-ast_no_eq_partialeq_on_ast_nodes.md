---
scope: valen-ast
severity: minor
dimension: idiomatic_rust
---

# No Eq Partialeq On Ast Nodes

## 概要

Most AST nodes lack PartialEq, making test assertions with assert_eq! impossible.

## 現状

crates/valen-ast/src/lib.rs

## 問題点

Limits test expressiveness.

## 改善案

Add #[derive(PartialEq)] to key types.

## 影響範囲

- crates/valen-ast/src/lib.rs

## 関連ファイル

(none)
