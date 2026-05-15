---
scope: valen-lsp
severity: major
dimension: security
---

# LSP workspace indexing follows symlinks outside workspace

## 概要

find_vln_files が Path::is_dir() で再帰するため、symlink 経由でワークスペース外のディレクトリを辿る。親ディレクトリやホームへの symlink があると、信頼境界外のファイルを解析したり、循環リンクでスタック枯渇する。

## 現状

crates/valen-lsp/src/server.rs:L1017-L1022: symlink_metadata ではなく metadata (= follow symlinks) を使用。

## 改善案

symlink_metadata/file_type で symlink ディレクトリを除外。canonical path がワークスペースルート配下であることを検証。visited ディレクトリの HashSet で循環検出。

## 影響範囲

- crates/valen-lsp/src/server.rs
