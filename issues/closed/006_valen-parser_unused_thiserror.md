---
scope: valen-parser
severity: minor
dimension: idiomatic_rust
---

# Unused thiserror and expect-test dependencies

## 概要
thiserror (依存) と expect-test (dev依存) が Cargo.toml に宣言されているが未使用。

## 影響範囲
- crates/valen-parser/Cargo.toml:L16, L19
