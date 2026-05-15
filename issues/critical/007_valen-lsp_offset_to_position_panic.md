---
scope: valen-lsp
severity: critical
dimension: correctness
---

# offset_to_position が範囲外バイトオフセットでパニック

## 概要

LineIndex::offset_to_position が line_start + byte_col でスライスする際、ソース長やライン境界へのクランプがない。

## 現状

crates/valen-lsp/src/convert.rs:

```rust
let line_text = &self.source[line_start..line_start + byte_col];
```

## 問題点

部分的に更新されたドキュメントからの stale Span や、範囲外のオフセットにより panic 発生。LSP サーバーはエディタ体験を壊さないために堅牢である必要がある。パニックはエディタの LSP プロセスをクラッシュさせる。

## 改善案

ガード追加: `let end = (line_start + byte_col).min(self.source.len());` — ライン境界（次の line_start またはソース末尾）へのクランプも実施。

## 影響範囲

- crates/valen-lsp/src/convert.rs

## 関連ファイル

なし
