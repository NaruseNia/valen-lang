---
scope: valenfmt
severity: major
dimension: correctness
---

# フォーマッタがファイルを非原子的に書き込む

## 概要

valenfmt は std::fs::write で元ファイルに直接書き込む。プロセス中断やファイルシステムエラー時に、ソースファイルが切り詰められた状態で残る可能性がある。

## 現状

crates/valenfmt/src/main.rs:L49: `std::fs::write(path, formatted)` — truncate then write で非原子的。

## 改善案

同一ディレクトリに一時ファイルを書き込み、fsync 後に atomic rename で上書き。パーミッション保持。一時ファイル書き込み失敗時は元ファイルを変更しない。

## 影響範囲

- crates/valenfmt/src/main.rs
