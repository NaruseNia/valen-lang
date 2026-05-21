---
scope: valen-parser
severity: major
dimension: correctness
---

# Self Parameter Operator Precedence Unclear

## 概要

Self parameter detection has ambiguous operator precedence (|| vs &&).

## 現状

crates/valen-parser/src/parser.rs:L347-L349

## 問題点

Works by accident, fragile to reordering.

## 改善案

Add explicit parentheses.

## 影響範囲

- crates/valen-parser/src/parser.rs:L347-L349

## 関連ファイル

(none)
