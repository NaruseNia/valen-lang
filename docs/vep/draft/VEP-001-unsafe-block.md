# VEP-001: unsafe block / unsafe fn

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

Java interop、FFI、unchecked cast、reflection、Panama 呼び出しなど、Valen の通常の型・失敗モデルでは保証できない操作を明示的に囲う `unsafe` ブロックおよび `unsafe fn` を導入する。

## 設計

`unsafe { ... }` ブロック内で型・失敗モデルの保証を bypass する操作を許可する。`unsafe fn` は呼び出し側にも `unsafe` コンテキストを要求する方向で検討。`safe {}` が Java exception / null の正規化を行うのに対し、`unsafe` はその正規化の bypass として位置づける。許可する操作を列挙制にするか責任境界にするかは要検討。
