---
scope: valen-hir
severity: major
dimension: correctness
---

# Orphan check uses import names not definition origin

## 概要
coherence.rs で foreign 判定が import リスト名前ベース。HIR の defs で local 定義かどうかを確認すべき。

## 改善案
HIR defs に対してローカル定義をチェックする方式に変更。

## 影響範囲
- crates/valen-hir/
