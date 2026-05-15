---
scope: valen-lsp
severity: enhancement
dimension: dependency
---

# LSP が tokio "full" feature を不要に引き込む

## 概要

ワークスペース Cargo.toml で tokio features = ["full"] を指定しているが、valen-lsp は #[tokio::main] と async-lsp stdio のみ使用。コンパイル時間と依存グラフを無駄に拡大。

## 改善案

tokio の feature を最小限（rt, macros 等）に絞り、ワークスペース test/build パイプラインで feature 依存の欠落がないか確認。

## 影響範囲

- Cargo.toml
- crates/valen-lsp/Cargo.toml
