---
scope: valen-codegen
severity: critical
dimension: error_handling
---

# emit_arith / emit_neg / lower_comparison の unreachable! が本番コードでパニック

## 概要

emit_arith、emit_neg、Object比較分岐で unreachable!() を使用。HIR から予期しない型が渡された場合に hard panic。

## 現状

crates/valen-codegen/src/emit.rs:

```rust
(op, ty) => unreachable!("emit_arith: unsupported {op:?} for {ty:?}"),
...
other => unreachable!("emit_neg: unsupported type {other:?}"),
```

## 問題点

HIR が予期しない型組み合わせ（Object に対する算術等）を生成した場合、コンパイラがパニック。将来の HIR 変更やパーサーバグで容易に到達可能。diagnostic を出す代わりにクラッシュする。

## 改善案

emit_arith, emit_neg を Result<Instruction, CodegenError> に変更。CodegenError に UnsupportedOperation variant を追加。unreachable! を全て Err(...) に置換。

## 影響範囲

- crates/valen-codegen/src/emit.rs
- crates/valen-codegen/src/expr.rs

## 関連ファイル

- crates/valen-codegen/src/jvm_ir.rs
