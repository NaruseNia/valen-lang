---
scope: valenc
severity: minor
dimension: correctness
---

# Warnings and hints silently swallowed

## 概要
診断出力が has_errors() == true の時のみ発生。warning/hint は一切表示されない。

## 改善案
常に診断を表示。has_errors() == true の場合のみ bail。

## 影響範囲
- crates/valenc/src/main.rs
