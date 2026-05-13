# VEP-031: Range / slice indexing

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

range 式とスライスインデックス構文を導入する。

## 設計

`xs[1..]` / `xs[..10]` / `xs[1..=10]` の形式で range ベースのインデックスアクセスを提供する。Java collection での slice が view か copy か、bounds error を panic / Result / Option のどれで扱うかが検討事項。
