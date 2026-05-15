---
scope: valen-hir
severity: major
dimension: correctness
---

# synth_path only resolves first segment of Enum::Variant path

## 概要
ty.rs synth_path で多セグメントパスの最初のセグメントのみ解決。Shape::Circle が Shape の型を返し ::Circle を無視。

## 改善案
enum variant パスの適切な処理を追加。EnumVariant TypedExprKind の新設を検討。

## 影響範囲
- crates/valen-hir/
