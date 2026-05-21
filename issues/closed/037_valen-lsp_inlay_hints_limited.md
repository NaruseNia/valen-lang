---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# Inlay Hints Limited

## 概要

Only let-type and lambda-param hints. Missing: call-site param names, chain return types. Known issue #7.

## 現状

crates/valen-lsp/src/server.rs:L2271-L2502

## 問題点

Only 2 of 5 expected hint categories.

## 改善案

Add parameter name hints and closure return type hints.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
