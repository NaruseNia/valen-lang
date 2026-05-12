---
scope: valen-hir
severity: minor
dimension: design
---

# resolve_primitive duplicated between resolve.rs and ty.rs

## 概要
プリミティブ型解決が2ファイルで同一ロジック重複。新しいプリミティブ追加時に両方更新必要。

## 改善案
lib.rs に共有関数として抽出。

## 影響範囲
- crates/valen-hir/src/resolve.rs:L389-L404
- crates/valen-hir/src/ty.rs:L1294-L1309
