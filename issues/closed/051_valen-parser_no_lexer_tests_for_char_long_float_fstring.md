---
scope: valen-parser
severity: major
dimension: test_coverage
---

# No Lexer Tests For Char Long Float Fstring

## 概要

Lexer tests lack coverage for char, Long (42L), Float (3.14f), f-string, underscore separators.

## 現状

crates/valen-parser/tests/lexer.rs

## 問題点

Key literal types untested.

## 改善案

Add snapshot tests for each literal type.

## 影響範囲

- crates/valen-parser/tests/lexer.rs

## 関連ファイル

(none)
