---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# No Impl Trait Only Context

## 概要

After typing 'impl ', all defs shown instead of only traits. Known issue #5.

## 現状

crates/valen-lsp/src/server.rs:L1636-L1687

## 問題点

No ImplTraitPosition context in detect_context.

## 改善案

Add CompletionContext::ImplTraitPosition variant.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
