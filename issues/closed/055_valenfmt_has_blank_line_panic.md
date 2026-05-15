---
scope: valenfmt
severity: major
dimension: error_handling
---

# has_blank_line panics when from > to

## 概要
source[from..to] スライスが from > to でパニック。AST span 順序異常やコメント位置のずれで発生可能。

## 改善案
ガード追加: if from >= to { return false; }。source.len() へのクランプも実施。

## 影響範囲
- crates/valenfmt/
