---
scope: valen-ast
severity: major
dimension: test_coverage
---

# No unit tests for AST types

## 概要
AST 型(lib.rs)と TokenKind(token.rs)のテスト皆無。Span テスト7件のみ。

## 改善案
Literal 構築テスト、TokenKind PartialEq テスト、span() メソッドテストを追加。

## 影響範囲
- crates/valen-ast/
