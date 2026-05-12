---
scope: valen-codegen
severity: minor
dimension: spec_coverage
---

# Missing SourceFile attribute in class files

## 概要
source_file が常に None。JVM スタックトレースが "Unknown Source" を表示し、デバッグを阻害。

## 改善案
HIR からソースファイル名を通し、SourceFile 属性を emit。

## 影響範囲
- crates/valen-codegen/src/lower.rs:L91
