---
scope: valen-ast
severity: enhancement
dimension: performance
---

# Large Expr Enum Size

## 概要

Expr enum has 30 variants; boxing more large variants could reduce enum size.

## 現状

crates/valen-ast/src/lib.rs:L409-L451

## 問題点

Inline variants inflate every Expr value.

## 改善案

Measure with size_of and box large variants.

## 影響範囲

- crates/valen-ast/src/lib.rs:L409-L451

## 関連ファイル

(none)
