---
scope: valen-codegen
severity: enhancement
dimension: performance
---

# Synthetic Classes Always Emitted

## 概要

ListIterator and 6 ref wrapper classes unconditionally emitted for every compilation.

## 現状

crates/valen-codegen/src/lower.rs:L70-L73

## 問題点

7 extra class files per module regardless of usage.

## 改善案

Track usage and emit only needed synthetic classes.

## 影響範囲

- crates/valen-codegen/src/lower.rs

## 関連ファイル

(none)
