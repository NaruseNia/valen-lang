# VEP-011: Refinement / newtype

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

`typealias` ではなく所有権を持つ軽量 wrapper `newtype` を導入する。orphan rule 上の所有の明確化、primitive obsession の回避、バリデーション済み値の型による区別が目的。

## 設計

`newtype UserId = Int` および `newtype Email = String where Email::is_valid` の形式。Valhalla value class との将来連携、runtime cost の仕様保証の是非、`derive` と組み合わせる trait 群の設計が検討事項。
