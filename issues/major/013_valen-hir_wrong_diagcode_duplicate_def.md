---
scope: valen-hir
severity: major
dimension: correctness
---

# Duplicate definition error uses NAME_NOT_FOUND code

## 概要
resolve.rs define_name() で重複名検出時に DiagCode::NAME_NOT_FOUND を使用。NAME_NOT_FOUND は意味的に矛盾（名前は見つかった、2回見つかった）。

## 改善案
DUPLICATE_DEFINITION コード（例: DiagCode(201)）を追加して使用。

## 影響範囲
- crates/valen-hir/
