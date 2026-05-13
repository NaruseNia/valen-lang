# VEP-008: Labeled block / early break

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

ループではないブロックから値付きで脱出する labeled block 構文を導入する。`return` より狭い早期脱出として機能する。

## 設計

`'label: { ... break 'label value; ... }` の形式でブロックに名前を付け、値付きで脱出する。`return` より狭いスコープの早期脱出としての導入価値、`try` ブロックとの二重化の回避が主な検討事項。
