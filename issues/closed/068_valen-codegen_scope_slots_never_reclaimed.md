---
scope: valen-codegen
severity: enhancement
dimension: performance
---

# Scope Slots Never Reclaimed

## 概要

Local variable slots never reclaimed on scope exit, inflating max_locals.

## 現状

crates/valen-codegen/src/expr.rs:L126-L155

## 問題点

Wasted JVM frame memory for deeply nested scopes.

## 改善案

Save/restore next_slot on scope entry/exit.

## 影響範囲

- crates/valen-codegen/src/expr.rs

## 関連ファイル

(none)
