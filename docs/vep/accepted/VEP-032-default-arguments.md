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
