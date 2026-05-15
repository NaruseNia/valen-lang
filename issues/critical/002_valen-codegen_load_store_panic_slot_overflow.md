---
scope: valen-codegen
severity: critical
dimension: correctness
---

# load/store instruction panic on slot > 255

## 概要

load_instruction / store_instruction が assert! で slot <= 255 を強制。本番コードでパニックする。

## 現状

crates/valen-codegen/src/emit.rs: `assert!(slot <= 255, "local slot {slot} exceeds u8 range; wide prefix not yet supported");`

```rust
fn load_instruction(slot: u16, ty: &JvmType) -> Instruction {
    assert!(
        slot <= 255,
        "local slot {slot} exceeds u8 range; wide prefix not yet supported"
    );
```

## 問題点

ネストしたラムダや wide 型（Long/Double）を多用するメソッドで128以上のローカル変数が使われた場合、コンパイラがパニックする。JVM には wide prefix 命令（Iload_w等）があるが未実装。graceful error ではなく hard panic。

## 改善案

Result<Instruction, CodegenError> を返すよう変更し、emit_op で ? 伝播。または wide prefix 命令サポートを実装。

## 影響範囲

- crates/valen-codegen/src/emit.rs

## 関連ファイル

- crates/valen-codegen/src/jvm_ir.rs
