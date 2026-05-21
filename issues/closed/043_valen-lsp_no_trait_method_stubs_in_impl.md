---
scope: valen-lsp
severity: major
dimension: spec_coverage
---

# No Trait Method Stubs In Impl

## 概要

Inside impl Trait for Type {}, no suggestion of unimplemented trait methods. Known issue #6.

## 現状

crates/valen-lsp/src/server.rs:L702

## 問題点

No impl-body detection or stub generation.

## 改善案

Detect impl block, look up trait methods, filter implemented, offer stubs.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
