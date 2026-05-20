---
scope: valen-lsp
severity: major
dimension: correctness
---

# Scope Filtering Insufficient

## 概要

Completions include defs from all scopes, not just visible at cursor. Known issue #10.

## 現状

crates/valen-lsp/src/server.rs:L750-L901

## 問題点

Out-of-scope items shown in completion.

## 改善案

Add scope-containment check for defs and fn parameters.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
