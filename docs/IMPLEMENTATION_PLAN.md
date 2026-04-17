# Valen 実装計画

Last updated: 2026-04-17

---

## Phase 0: 基盤整備（1-2 週間）

**目的:** 後続 Phase 全体の土台を整える。

### タスク
- [ ] Rust workspace 初期化（`compiler/`, `lsp/`, `fmt/`）
- [ ] JVM classfile crate 選定と PoC
  - 候補：[noak](https://crates.io/crates/noak), [cafebabe](https://crates.io/crates/cafebabe), [classfile-parser](https://crates.io/crates/classfile-parser)
  - 評価基準：write API の完成度、ClassFile emit の対応範囲、Java 21/25 bytecode サポート、メンテナンス
- [ ] **enum bytecode emit の実験 spike**：sealed interface + record + singleton の組み合わせを手書きで emit し、以下を検証
  - Java 21 `switch` pattern matching での exhaustive
  - Jackson / Gson での serialize/deserialize
  - `java.lang.reflect` での class 名前解決
  - Gradle incremental compilation
- [ ] Parser 戦略の選定（手書き recursive descent vs parser generator）
- [ ] AST / IR の設計
- [ ] テストインフラ（snapshot テスト、integration テスト、bytecode diff）

### 完了条件
- 最小限の `class Foo {}` を `.vln` → `Foo.class` に emit できる PoC が動く
- 上記 spike 結果が `docs/enum-abi-report.md` にまとまっている

---

## Phase 1: MVP（3-6 ヶ月）

**目的:** Valen の核（ADT / match / trait / Option/Result）を最小限で動かし、Java 相互運用できる状態にする。

### 言語機能

| カテゴリ | 機能 |
|---------|------|
| 基本 | `fn`（top-level）, `let` / `let mut`, 式指向（末尾式） |
| クラス | `class`, primary constructor, `data class`（equals/hashCode/toString/copy 自動生成） |
| 継承 | `open` / `abstract` / `sealed`、single inheritance + trait multiple |
| enum | Rust ADT、payload あり/なし、`::` variant アクセス |
| match | フルセット（リテラル / 構造分解 / ガード / 範囲 / or / `@`束縛）、Valen enum exhaustive |
| trait | trait 定義、impl、inherent impl、UFCS |
| coherence | orphan rule 厳格、blanket impl 禁止、一意性保証 |
| 失敗 | `Option` / `Result` / `panic`、`?` 演算子（Result + Option） |
| Java 相互運用 | `import`, `safe { ... }` ブロック（exception → Result 明示変換）, Java 型は foreign |
| 可視性 | `pub` / `internal` / `private` |
| 引数 | 位置引数 + 名前付き引数 |
| 文字列 | `f"{expr}"`（単行のみ） |
| for | `for x in iter`、Iterator trait、java.lang.Iterable 自動アダプト |
| 型 | プリミティブ名義型、ジェネリクス（erasure）、typealias |
| 並行 | virtual thread のみ |

### ツール

| ツール | 実装 | 範囲 |
|--------|-----|------|
| valenc | Rust | AST → bytecode、エラー診断、Java import 解決 |
| Gradle plugin | Kotlin | `compileValen` タスク、標準 sourceSet 対応 |
| LSP | Rust | syntax エラー表示、型エラー診断、goto definition |
| valenfmt | Rust | brace style、indent、trailing `;` の自動整形（最小版） |

### 標準ライブラリ

- `valen.core`: `Option<T>`, `Result<T, E>`, `Iterator<T>` trait
- `valen.collections`: `List<T>` / `Map<K, V>` / `Set<T>` は `java.util` の typealias、trait で iteration + map + filter 注入
- `valen.io`: 基本 IO ラッパ（safe ブロックで Java IOException → Result 変換）

### サンプルプロジェクト
- `examples/hello/`（Hello World）
- `examples/shapes/`（ADT + match + trait + Gradle build）
- `examples/java-interop/`（Java クラス呼び出し、exception 明示変換）

### 完了条件
- 上記サンプル3本が Gradle + valen-gradle-plugin で build 可能
- LSP が VSCode 拡張経由で動作、syntax エラーがリアルタイム表示される
- enum bytecode emit 戦略が Phase 0 の spike 結果を反映し、Jackson / reflection / pattern switch で問題なく動作
- セルフホストの足がかり：Valen で簡単な CLI が書ける

---

## Phase 1.5: MVP 補完（2-3 ヶ月）

**目的:** MVP で削った「Java/Kotlin ユーザ向け快適機能」を順次追加。

### 言語機能追加
- デフォルト引数
- `inline fn`（reified は Phase 2）
- 演算子オーバーロード（trait ベース、Add/Sub/Mul/Div/Index）
- 文字列補間 multiline `f"""..."""`
- DSL receiver lambda（`T.() -> Unit`）再評価
  - 採用するなら：**仕様上 extension とは完全に別物として隔離**（名前解決は lambda 型糖衣であり拡張ではない）
- annotation consumption（読み取り側、`@Annotation` 定義と読み取り）
- reflection 統合（`java.lang.reflect` のラッパ）
- Java sealed hierarchy への exhaustive 判定（`@closed` アノテーション認識）
- Java collection への trait 注入の厚みを増す（reduce, fold, groupBy 等）

### ツール拡張
- LSP：補完、リファクタリング、hover type、signature help
- valenfmt：多くの rule 追加、IDE 連携
- 診断メッセージの充実

### 完了条件
- Kotlin からの移植コストが「大きな違和感なし」レベル
- VSCode 拡張で Java/Kotlin IDE 並の日常開発体験

---

## Phase 2: 高度機能（3-6 ヶ月）

**目的:** 大規模アプリ・ライブラリ開発に耐える完成度へ。

### 言語機能
- `suspend fn` / async モデル再評価
  - virtual thread との統合戦略を仕様化
- `reified` 型パラメータ
- annotation processing（processor API、KSP 相当）
- 独自 read-only / mutable collection façade（`valen.collections.v2`）
  - `java.util` typealias からの移行パス

### ツール
- Maven プラグイン
- 診断メッセージのさらなる改善
- LSP の inlay hint、semantic token

### 完了条件
- Spring Boot / Ktor 相当のフレームワークを Valen で書ける
- Valen ライブラリが Maven Central で公開可能

---

## Phase 3: 開発者体験（2-3 ヶ月）

**目的:** 学習・実験を加速。

### ツール
- REPL（`valen repl`）、JShell より優れた ADT / match の対話体験
- Playground（ブラウザで試せる）

### ドキュメント
- 公式ガイド
- 言語リファレンス
- Java/Kotlin からの移行ガイド

---

## Phase 4+（凍結中）

- **valen-pkg（cargo 相当）**：独自 package manager + registry 構想は当面凍結
  - Gradle / Maven 完全従属で十分と判断（Codex レビュー採用）
  - 将来、Valen 独自の依存解決 / lock file / cross-compile が必要になった時点で解凍

---

## リスクと対策

| リスク | 対策 |
|-------|------|
| enum bytecode emit の罠 | Phase 0 で実験 spike、検証結果を仕様に反映してから MVP 着手 |
| coherence 仕様の抜け | Phase 1 で実装しながら判例を蓄積、`docs/coherence-rules.md` で明文化 |
| Java interop の boxing 予期外 | Phase 0 で overload resolution 規則を確定し、`docs/java-overload.md` に記述 |
| LSP 性能 | Phase 1 から incremental parsing を前提に設計、salsa crate 等検討 |
| セルフホスト負債 | Phase 2 までは Rust で保守、Phase 3 以降で段階的に Valen 化を検討 |

---

## 指標（KPI 候補）

- Phase 1 完了時：Hello World + ADT + Java interop の3サンプルが Gradle で build 可能
- Phase 1.5 完了時：GitHub で外部コントリビュータが PR を送り始める
- Phase 2 完了時：Maven Central に最初の Valen ライブラリが公開
- Phase 3 完了時：月間アクティブ開発者 100+ 人

---

## コントリビュータ

- [あなた](https://github.com/NaruseNia) — 設計・初期実装

## ライセンス

Apache License 2.0
