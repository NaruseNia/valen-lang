---
scope: valen-codegen
severity: critical
dimension: correctness
---

# IInc i32-to-i16 silent truncation

## 概要

JvmOp::IInc の increment 値が i16 範囲外の場合、`*inc as i16` で無言に切り詰められ、不正なバイトコードが生成される。

## 現状

crates/valen-codegen/src/emit.rs:

```rust
JvmOp::IInc(slot, inc) => {
    if *slot <= 255 && (-128..=127).contains(inc) {
        vec![Instruction::Iinc(*slot as u8, *inc as i8)]
    } else {
        vec![Instruction::Iinc_w(*slot, *inc as i16)]
    }
}
```

## 問題点

JvmOp::IInc は inc を i32 で保持するが、Iinc_w は i16 (-32768..32767) のみサポート。範囲外の値で `as i16` が無言に truncate し、誤ったインクリメント値のバイトコードを生成。現在は increment 1 のみ使用されるが、IR の型が i32 を許容している。

## 改善案

JvmOp::IInc の inc を i16 に変更するか、範囲外で load-add-store シーケンスにフォールバック。または CodegenError を返す。

## 影響範囲

- crates/valen-codegen/src/emit.rs

## 関連ファイル

- crates/valen-codegen/src/jvm_ir.rs
