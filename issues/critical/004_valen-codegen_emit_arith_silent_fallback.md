---
scope: valen-codegen
severity: critical
dimension: correctness
---

# emit_arith silent fallback to iadd for invalid types

## 概要

emit_arith がサポート外の型（Object, Array, Void）に対して黙って Iadd にフォールバックし、不正なバイトコードを生成する。

## 現状

crates/valen-codegen/src/emit.rs:L702

```rust
_ => Instruction::Iadd, // fallback
```

## 問題点

不正な型での算術演算がコンパイルエラーにならず、JVM 検証エラーまたは実行時データ破損を引き起こす。

## 改善案

フォールバックを `unreachable!("emit_arith called with unsupported type: {ty:?}")` に置き換える。

## 影響範囲

- crates/valen-codegen/src/emit.rs:L702
