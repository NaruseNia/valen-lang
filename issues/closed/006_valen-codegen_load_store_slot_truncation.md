---
scope: valen-codegen
severity: critical
dimension: correctness
---

# load/store slot truncation u16 to u8

## 概要

load_instruction と store_instruction が u16 スロットインデックスを `as u8` で切り捨て。スロット >= 256 で誤ったローカル変数を参照する不正バイトコードが生成される。

## 現状

crates/valen-codegen/src/emit.rs:L766-L848

```rust
s => Instruction::Iload(s as u8),
```

## 問題点

スロット 256 が 0 にラップし、間違ったローカル変数を参照する。

## 改善案

スロット > 255 の場合は `wide` プレフィックスを emit するか、最低限 debug_assert を追加。

## 影響範囲

- crates/valen-codegen/src/emit.rs:L766-L848
