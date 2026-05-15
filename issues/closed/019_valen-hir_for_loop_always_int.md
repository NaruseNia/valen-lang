---
scope: valen-hir
severity: major
dimension: correctness
---

# for loop variable defaults to Int for non-Range types

## 概要
ty.rs synth_for で Range 以外の全型がデフォルト Ty::Prim(PrimTy::Int)。List<String> のイテレーションで変数が Int に。

## 改善案
Iterator impl から要素型を抽出。未知型は Ty::Error + diagnostic。

## 影響範囲
- crates/valen-hir/
