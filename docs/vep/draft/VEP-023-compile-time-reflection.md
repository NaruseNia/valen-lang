# VEP-023: Compile-time reflection

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

型・field・variant 情報をコンパイル時に読む API を導入する。serializer 生成、database mapper、exhaustive UI renderer が主な用途。

## 設計

コンパイル時に型のメタデータ（field 名・型・variant 一覧等）へアクセスする API を提供する。Java reflection と Valen metadata の二重化の回避、private member の読み取り範囲、incremental compilation への影響が検討事項。
