---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Missing Hex Binary Octal Integer Literals

## 概要

Lexer does not support 0xFF, 0b1010, 0o77 prefixes. User-reported.

## 現状

crates/valen-parser/src/lexer.rs:L235-L244

## 問題点

Only decimal integer regex defined.

## 改善案

Add regex for hex/binary/octal with from_str_radix.

## 影響範囲

- crates/valen-parser/src/lexer.rs:L235-L244

## 関連ファイル

(none)
