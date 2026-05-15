---
scope: valen-lsp
severity: major
dimension: correctness
---

# find_let_type_annotation returns early on first non-let line

## 概要
strip_prefix に ? 演算子使用でループ初回の非 let 行で None を返す。let 以外の行が先に来るとほぼ常に None。

## 改善案
? を let-else / match+continue パターンに置換。

## 影響範囲
- crates/valen-lsp/
