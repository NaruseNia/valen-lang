---
scope: valenfmt
severity: critical
dimension: correctness
---

# Unsafe Expr Missing From Expr Ends With Block

## 概要

Expr::Unsafe not in expr_ends_with_block(). Formatter adds spurious semicolon after unsafe blocks.

## 現状

crates/valenfmt/src/printer.rs:L1446-L1459

## 問題点

Semicolon changes expression semantics (value discarded).

## 改善案

Add Expr::Unsafe(_) to the match.

## 影響範囲

- crates/valenfmt/src/printer.rs:L1446-L1459

## 関連ファイル

(none)
