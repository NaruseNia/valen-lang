---
scope: valen-codegen
severity: major
dimension: error_handling
---

# VerificationError strips StackMapTable without re-verifying

## 概要

verify() が VerificationError を返すと StackMapTable 属性を全除去するが、Java 21 は StackMapTable を要求するため JVM がロード時に拒否する。

## 改善案

StackMapTable 生成を修正するか、最低限どのクラスが除去されたかログ出力。

## 影響範囲

- crates/valen-codegen/src/emit.rs:L114-L127
