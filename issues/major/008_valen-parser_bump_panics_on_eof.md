---
scope: valen-parser
severity: major
dimension: correctness
---

# bump() panics on EOF (index out of bounds)

## 概要

bump() が直接インデックス `self.tokens[self.pos]` を使用。トークンストリーム末尾を超えて呼ばれるとパニック。

## 改善案

`self.tokens.get(self.pos).cloned()` で合成 Eof トークンにフォールバック。

## 影響範囲

- crates/valen-parser/src/parser.rs:L1322-L1326
