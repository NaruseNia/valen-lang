---
scope: valen-codegen
severity: enhancement
dimension: performance
---

# String Concat Not Using Invokedynamic

## 概要

F-string uses StringBuilder instead of JDK 9+ makeConcatWithConstants invokedynamic.

## 現状

crates/valen-codegen/src/expr.rs:L3197-L3237

## 問題点

More bytecode, slower for small interpolations.

## 改善案

Emit invokedynamic with StringConcatFactory bootstrap.

## 影響範囲

- crates/valen-codegen/src/expr.rs

## 関連ファイル

(none)
