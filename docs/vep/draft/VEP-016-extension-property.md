# VEP-016: Extension property

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

trait method に加えて、読み取り専用 property 風の拡張を許す extension property を導入する。

## 設計

`trait HasLength { prop length(self) -> Int; }` の形式で property 風アクセスを定義する。Kotlin の extension property と同じ錯覚を生まないこと、実体は method であることを仕様上明確にすることが検討事項。
