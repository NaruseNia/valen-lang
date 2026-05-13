# VEP-006: when expression

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

`match` より軽い条件分岐、または値を取らない exhaustive 分岐として使う `when` 式を導入する。

## 設計

Kotlin 風の条件列挙 `when { cond => expr }` または `match` の糖衣 `when value { pattern => expr }` の 2 案がある。`match` との役割の重なり、`else` vs `_` の選択、exhaustive check の対象とするかが主な検討事項。guard 付き `match` で十分な可能性もあるため慎重に評価する。
