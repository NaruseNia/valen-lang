# VEP-013: derive

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 2 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

構造から明らかな trait 実装を自動生成する `derive` 機構を導入する。`Eq` / `Hash` / `Display` / `Clone` / `Ord` 等の trait が対象候補。

## 設計

`#[derive(Eq, Hash, Display, Clone)]` の形式で struct / enum に trait 実装を自動生成する。annotation 構文 `@` と Rust 風 `#[...]` の選択、derive macro まで開くか builtin derive のみにするか、Java `equals` / `hashCode` / `toString` との完全連動、orphan rule と coherence への組み込みが検討事項。
