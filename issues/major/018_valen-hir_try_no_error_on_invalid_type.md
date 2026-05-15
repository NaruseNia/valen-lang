---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# ? operator silently passes on non-Option/Result types

## 概要
ty.rs synth_try のフォールバック _ => (inner.ty.clone(), false) が任意の型を通過。42? が型エラーにならない。

## 改善案
_ arm で diagnostic error を出し Ty::Error を返す。

## 影響範囲
- crates/valen-hir/
