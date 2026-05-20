---
scope: valen-hir
severity: critical
dimension: correctness
---

# Inherent Impl Blocks From Stdlib Not Registered

## 概要

Inherent impl blocks (impl Int, impl Option<T>, impl Result<T,E>) from core.vln silently dropped. toLong(), Option::map() etc never available.

## 現状

crates/valen-hir/src/resolve.rs:L161-L181

## 問題点

trait_name is None for inherent impls, causing both_injected=false unconditionally.

## 改善案

Handle inherent impls separately: require only target_name in injected_names.

## 影響範囲

- crates/valen-hir/src/resolve.rs:L161-L181

## 関連ファイル

- stdlib/valen/core/core.vln
