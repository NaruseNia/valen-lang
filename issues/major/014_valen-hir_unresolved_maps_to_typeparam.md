---
scope: valen-hir
severity: major
dimension: correctness
---

# TyRef::Unresolved blindly maps to Ty::TypeParam

## 概要
lib.rs tyref_to_ty で TyRef::Unresolved(n) → Ty::TypeParam(n) にマップ。タイポが型パラメータとして扱われ偽陽性の型チェック通過。

## 改善案
TyRef::Unresolved を Ty::Error にマップし、ジェネリクス文脈でのみ TypeParam 扱い。

## 影響範囲
- crates/valen-hir/
