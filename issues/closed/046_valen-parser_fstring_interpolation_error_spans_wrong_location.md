---
scope: valen-parser
severity: major
dimension: correctness
---

# Fstring Interpolation Error Spans Wrong Location

## 概要

F-string interpolation parse errors point to entire f-string, not the specific broken expression.

## 現状

crates/valen-parser/src/parser.rs:L2351-L2358

## 問題点

Reparsing from synthetic source loses original positions.

## 改善案

Compute sub-span based on { offset within f-string.

## 影響範囲

- crates/valen-parser/src/parser.rs:L2351-L2358

## 関連ファイル

(none)
