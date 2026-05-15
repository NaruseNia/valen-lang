---
scope: valenc
severity: major
dimension: spec_coverage
---

# --target flag parsed but never forwarded to codegen

## 概要
Compile variant が --target を受け取るが destructure 時に .. で破棄。compile_hir は常に Java 21。

## 改善案
target を JvmVersion にパースし compile_hir に渡す。

## 影響範囲
- crates/valenc/
