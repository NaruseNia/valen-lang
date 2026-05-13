# VEP-022: Hygienic macro

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

AST ベースの hygienic macro を導入する。文字列置換は禁止し、衛生的なマクロ展開のみ許可する。

## 設計

`macro name(params) { ... }` の形式で AST レベルのマクロを定義する。compile-time API の安定性、IDE / LSP が展開前後をどう扱うか、derive だけで足りる範囲の見極めが検討事項。特に慎重に扱う機能として分類されている。
