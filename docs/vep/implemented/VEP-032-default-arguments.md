# VEP-032: Default arguments

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 1.5 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

関数パラメータにデフォルト値を指定できる default arguments を導入する。MVP では named arguments のみだった機能を拡張する。

## 設計

Phase 1.5 で named arguments と組み合わせて導入する。Java 側からの呼び出し時の overload 生成戦略、default 式の評価タイミング（call-site vs definition-site）が検討事項。

## 実装済み設計決定（Phase 1.5 TASK-027）

| 項目 | 決定 |
|------|------|
| 評価タイミング | call-site（Kotlin 方式） |
| 式の範囲 | 任意の式 |
| 位置制約 | 任意位置。named args で中間パラメータも省略可 |
| ctor パラメータ | class / data class 両方で許可 |
| trait メソッド | 対応。impl での上書きは不可 |
| Java 互換 | Kotlin 式 synthetic（`$default` + ビットマスク）— codegen は Phase 2 |
