---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Missing block comment /* */ lexing

## 概要

仕様 §1.4 でブロックコメント `/* ... */` が定義されるが、lexer は `//` 単行コメントのみスキップ。ブロックコメントのあるコードは Error トークンを生成する。

## 改善案

logos skip ディレクティブにブロックコメントパターンを追加。

## 影響範囲

- crates/valen-parser/src/lexer.rs:L19
