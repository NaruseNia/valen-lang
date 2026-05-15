---
scope: valenc
severity: major
dimension: correctness
---

# Diagnostics attributed to first input file only

## 概要
複数ファイルコンパイル時、全 diagnostics が first_path + first_line_idx で emit。2番目以降のファイルのエラーが誤ったファイル名・位置で表示。

## 改善案
FileId ベースで正しい (path, LineIndex) ペアを参照。SourceMap 構造体導入。

## 影響範囲
- crates/valenc/
