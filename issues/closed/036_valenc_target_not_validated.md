---
scope: valenc
severity: major
dimension: error_handling
---

# --target value not validated

## 概要
target フィールドが String で任意値を受容。clap ValueEnum で 21/25 に制限すべき。

## 改善案
clap value_parser で有効値を制限。

## 影響範囲
- crates/valenc/
