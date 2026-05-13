# VEP-014: sealed trait

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 1.5 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

trait 実装集合を閉じ、trait 上の exhaustive match を許す `sealed trait` を導入する。

## 設計

`sealed trait Expr` のように宣言し、同一モジュール内の impl のみを許可する。enum と役割の重なりを制御し、Java sealed interface と ABI を揃える方向。downstream crate / module での impl 禁止の表現方法が検討事項。
