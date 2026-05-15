---
scope: valen-codegen
severity: major
dimension: correctness
---

# StackMapTable stripped on verification failure

## 概要
emit.rs でクラスファイル検証失敗時に全 StackMapTable を削除して続行。JVM 50.0+ で StackMapTable 必須のため実行時 VerifyError。

## 改善案
正しい StackMapTable 生成を修正するか、CodegenError を返す。

## 影響範囲
- crates/valen-codegen/
