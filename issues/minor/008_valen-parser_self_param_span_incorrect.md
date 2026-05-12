---
scope: valen-parser
severity: minor
dimension: correctness
---

# self parameter type span incorrect for mut self

## 概要
`mut self` パース時、合成 Self 型の span が `mut` キーワードを指す。`self` キーワードの span であるべき。

## 影響範囲
- crates/valen-parser/src/parser.rs:L177-L197
