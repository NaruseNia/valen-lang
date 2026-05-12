---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# No mutability check on assignment

## 概要

synth_assign がアサインメント先の可変性を検証しない。不変変数への再代入がエラーにならない。

## 改善案

TypeEnv を (Ty, bool) に拡張し mutability を追跡。synth_assign で不変先への代入を報告。

## 影響範囲

- crates/valen-hir/src/ty.rs:L1041-L1052
