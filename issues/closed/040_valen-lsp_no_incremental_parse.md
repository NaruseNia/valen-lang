---
scope: valen-lsp
severity: major
dimension: performance
---

# Full re-parse on every keystroke

## 概要
didChange で毎回フルパイプライン（parse+resolve+coherence+exhaustive+type_check）を同期実行。デバウンスなし。

## 改善案
100-200ms デバウンスを追加。中期的にインクリメンタルパーシング導入。

## 影響範囲
- crates/valen-lsp/
