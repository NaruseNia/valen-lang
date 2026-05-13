# VEP-017: JDK 25 first-class target

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

JDK 25 を単なる opt-in ではなく一級ターゲットとして扱う。JDK 25 世代の JVM 機能（Scoped Values、Structured Concurrency、Stable Values 等）を Valen の設計に取り込む。

## 設計

`--target 21`（互換 baseline）/ `--target 25`（first-class target）/ `--target latest`（実験用）の 3 段階を提供する。JDK 25 の preview / incubator 機能への依存を安定仕様に含めるか target-specific optimization に留めるか、JDK 21 と 25 で ABI が分岐する場合の Java 互換性維持が主な検討事項。
