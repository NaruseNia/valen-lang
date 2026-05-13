# VEP-025: async / await

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

JVM virtual thread を baseline としつつ、非同期境界を型で表す async / await 構文を導入するかを検討する。

## 設計

virtual thread で十分な領域と structured concurrency が必要な領域を分離した上で、型レベルの非同期境界を設計する。`Result` と cancellation の関係、Java `CompletableFuture` / reactive library との相互運用が検討事項。便利機能だが二重化に注意する機能として分類されている。
