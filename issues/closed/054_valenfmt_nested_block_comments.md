---
scope: valenfmt
severity: major
dimension: correctness
---

# Nested block comments cause incorrect extraction

## 概要
extract_comments がネスト深度を追跡しない。/* /* inner */ */ で最初の */ で停止し残りがコードとして扱われる。

## 改善案
深度カウンタ追加: /* でインクリメント、*/ でデクリメント、深度 0 で break。

## 影響範囲
- crates/valenfmt/
