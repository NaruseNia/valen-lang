---
scope: valenc
severity: major
dimension: correctness
---

# Emit Diagnostics Fallback Wrong File

## 概要

FileId out of range falls back to first file, showing errors against wrong file.

## 現状

crates/valenc/src/main.rs:L185-L191

## 問題点

Misleading diagnostics pointing to wrong file.

## 改善案

Use '<unknown>' placeholder instead of fallback.

## 影響範囲

- crates/valenc/src/main.rs

## 関連ファイル

(none)
