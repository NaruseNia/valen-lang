---
scope: valen-parser
severity: major
dimension: test_coverage
---

# No Parser Tests For Unsafe Safe Cast Deref Refmut

## 概要

No parser tests for unsafe, safe, as-cast, deref, ref mut despite being implemented.

## 現状

crates/valen-parser/tests/parser.rs

## 問題点

M14 features lack parser-level tests.

## 改善案

Add snapshot tests for each construct.

## 影響範囲

- crates/valen-parser/tests/parser.rs

## 関連ファイル

(none)
