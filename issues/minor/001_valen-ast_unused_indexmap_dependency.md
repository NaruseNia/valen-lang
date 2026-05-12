---
scope: valen-ast
severity: minor
dimension: idiomatic_rust
---

# Unused indexmap dependency

## 概要
Cargo.toml に indexmap が宣言されているが、ソースで未使用。

## 影響範囲
- crates/valen-ast/Cargo.toml:L12
