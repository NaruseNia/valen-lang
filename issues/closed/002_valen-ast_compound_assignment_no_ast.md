---
scope: valen-ast
severity: minor
dimension: spec_coverage
---

# Compound assignment tokens exist but no AST representation

## 概要
TokenKind に PlusEq/MinusEq 等があるが AssignExpr に operator フィールドがない。`x += 1` の忠実な AST 表現が不可。

## 改善案
AssignExpr に `op: Option<BinaryOp>` を追加。

## 影響範囲
- crates/valen-ast/src/lib.rs:L406-L411
