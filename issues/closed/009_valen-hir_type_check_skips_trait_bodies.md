---
scope: valen-hir
severity: minor
dimension: spec_coverage
---

# Type checker and exhaustiveness skip trait default method bodies

## 概要
check_items が Fn, Class, Impl のみ処理。Trait デフォルトメソッド本体は型チェックも exhaustiveness チェックもされない。

## 改善案
Item::Trait arm を追加。

## 影響範囲
- crates/valen-hir/src/ty.rs:L96-L107
- crates/valen-hir/src/exhaustive.rs:L31-L52
