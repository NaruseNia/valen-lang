---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# No Java Stdlib Completion

## 概要

Zero Java type/method completion. Known issue #11.

## 現状

crates/valen-lsp/src/server.rs:L1435

## 問題点

No Java class definitions in completion.

## 改善案

Create Java stdlib index (java.lang.*, java.util.*, java.io.*).

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
