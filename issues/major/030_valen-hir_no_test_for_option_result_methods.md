---
scope: valen-hir
severity: major
dimension: test_coverage
---

# No Test For Option Result Methods

## 概要

No tests for Option::map(), Result::map(), unwrapOr() etc. Core failure model untested.

## 現状

crates/valen-hir/src/ty.rs tests

## 問題点

Method resolution bugs on core types go undetected.

## 改善案

Add tests for Option/Result method chains.

## 影響範囲

- crates/valen-hir/src/ty.rs

## 関連ファイル

(none)
