---
scope: valen-hir
severity: major
dimension: error_handling
---

# TyRef uses Debug format in user-facing diagnostics

## 概要

coherence と resolve が TyRef を {:?} (Debug) でフォーマット。エラーメッセージに `Prim(Int)` のようなノイズが出る。

## 改善案

TyRef に Display を実装し `{}` を使用。

## 影響範囲

- crates/valen-hir/src/coherence.rs:L215-L220
- crates/valen-hir/src/resolve.rs:L94
