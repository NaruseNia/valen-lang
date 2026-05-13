---
scope: valen-hir
severity: major
dimension: correctness
---

# is_type_param heuristic is fragile

## 概要

blanket impl 検出が単一大文字名ヒューリスティックで型パラメータを判定。TK, Key 等の複数文字ジェネリクスを見逃す。

## 改善案

lowering 時にジェネリクス宣言から型パラメータを明示的に追跡。

## 影響範囲

- crates/valen-hir/src/coherence.rs:L247-L260
