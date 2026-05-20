---
scope: valen-codegen
severity: major
dimension: spec_coverage
---

# Main Fn Not Emitted As Jvm Entry Point

## 概要

No special handling of fn main() as JVM entry point. main()V emitted instead of main([Ljava/lang/String;)V.

## 現状

crates/valen-codegen/src/lower.rs

## 問題点

Compiled programs unrunnable. User-reported.

## 改善案

Generate wrapper class with JVM-compatible main signature.

## 影響範囲

- crates/valen-codegen/src/lower.rs

## 関連ファイル

(none)
