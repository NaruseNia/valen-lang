# VEP-024: const eval

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

コンパイル時に純粋式を評価する `const` 機構を導入する。

## 設計

`const PAGE_SIZE: Int = 1024 * 4;` の形式でコンパイル時定数を定義する。許可する式の範囲、panic 発生時の compile error 化、Java `static final` との対応が検討事項。
