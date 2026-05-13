# VEP-015: Specialization / default impl

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

generic trait impl の上に、より具体的な型向け実装を許す specialization を導入する。

## 設計

default impl を持つ generic trait impl を、より具体的な型で上書き可能にする。coherence を壊しやすい点、JVM dispatch で説明可能かどうか、Rust の specialization と同じ不安定さを持ち込まないかが主な検討事項。特に慎重に扱う機能として分類されている。
