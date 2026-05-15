---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# Classpath scanner が JAR ファイルを処理しない

## 概要

CLI は --classpath にディレクトリまたは JAR を受け取ると文書化しているが、load_class_from_classpath は常にファイルシステムパスとして結合。JAR classpath エントリは silent に失敗し、Java interop の一般的ケースが動作しない。

## 現状

crates/valen-hir/src/classpath.rs:L43-L50: パス結合して fs::read のみ。ZIP/JAR リーダー不在。

## 改善案

.jar/.zip 拡張子を検出して ZIP リーダー経由でクラスエントリを読み込み。未サポートや読取不能な classpath エントリには diagnostic を返す。

## 影響範囲

- crates/valen-hir/src/classpath.rs
- crates/valenc/src/main.rs
