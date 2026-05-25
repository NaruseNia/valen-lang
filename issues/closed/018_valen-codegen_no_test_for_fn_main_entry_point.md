---
scope: valen-codegen
severity: major
dimension: test_coverage
---

# No Test For Fn Main Entry Point

## 概要

No test verifies fn main() produces valid JVM entry point.

## 現状

crates/valen-codegen/tests/e2e_fixtures.rs

## 問題点

Most basic use case untested.

## 改善案

Create fn_main.vln fixture and verify main descriptor.

## 影響範囲

- crates/valen-codegen/tests/e2e_fixtures.rs

## 関連ファイル

(none)
