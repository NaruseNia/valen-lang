---
scope: valen-hir
severity: major
dimension: performance
---

# Method Resolution Linear Scan All Defs

## 概要

Multiple hot-path operations scan all HIR defs linearly. O(n) per method call/field access.

## 現状

crates/valen-hir/src/lib.rs:L61-L108, ty.rs:L218-L260

## 問題点

Bottleneck for moderate-sized programs.

## 改善案

Build name-to-DefId index for O(1) lookups.

## 影響範囲

- crates/valen-hir/src/lib.rs

## 関連ファイル

(none)
