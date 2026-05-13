---
scope: valen-diagnostics
severity: major
dimension: test_coverage
---

# valen-diagnostics: Zero test coverage

## 概要

diagnostics crate にテストが0件。error()/warning() の severity、has_errors()、len()/is_empty() が未検証。

## 改善案

基本的な動作テストを追加。

## 影響範囲

- crates/valen-diagnostics/src/lib.rs
