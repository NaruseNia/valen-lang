---
scope: valenc
severity: critical
dimension: correctness
---

# Target Jvm Version Silently Ignored

## 概要

--target flag parsed but value silently discarded. Always generates Java 21.

## 現状

crates/valenc/src/main.rs:L249

## 問題点

--target 25 produces Java 21 classfile with no warning.

## 改善案

Warn when target 25 not yet supported. Pass to compile_hir when API ready.

## 影響範囲

- crates/valenc/src/main.rs

## 関連ファイル

- crates/valen-codegen/src/lib.rs
