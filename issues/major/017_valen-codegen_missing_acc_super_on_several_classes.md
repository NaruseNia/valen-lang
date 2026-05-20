---
scope: valen-codegen
severity: major
dimension: correctness
---

# Missing Acc Super On Several Classes

## 概要

Newtype, ListIterator, ref wrapper classes lack ACC_SUPER flag. Required by JVM spec.

## 現状

crates/valen-codegen/src/lower.rs

## 問題点

invokespecial may resolve incorrectly in inheritance.

## 改善案

Set is_super: true for all non-interface classes.

## 影響範囲

- crates/valen-codegen/src/lower.rs

## 関連ファイル

(none)
