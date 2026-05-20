---
scope: valen-hir
severity: minor
dimension: spec_coverage
---

# Classpath Scan Only Java Javax Org

## 概要

scan_classpath only loads java/, javax/, org/ prefixes. com.*, net.*, io.* silently ignored.

## 現状

crates/valen-hir/src/classpath.rs:L28-L31

## 問題点

Java interop broken for non-standard package prefixes.

## 改善案

Remove prefix filter or make configurable.

## 影響範囲

- crates/valen-hir/src/classpath.rs:L28-L31

## 関連ファイル

(none)
