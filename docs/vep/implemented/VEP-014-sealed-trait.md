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

## 実装済み設計決定（Phase 1.5 TASK-029）

| 項目 | 決定 |
|------|------|
| 実装者の型制約 | `class` + `data class` のみ。enum 不可 |
| ジェネリクス | 許可。exhaustive check は型パラメータ無視（erasure） |
| 実装者の宣言構文 | `impl SealedTrait for Type { ... }`（trait 統一） |
| default method | 非対応（通常 trait と同じ制約） |
| module スコープ | 同一コンパイル単位で緩和。module 基盤は将来対応 |
| JVM ABI | sealed interface（ACC_INTERFACE + PermittedSubclasses） |
| supertrait | 非対応 |
| マーカー trait | 許可（メソッド0個でも有効） |
