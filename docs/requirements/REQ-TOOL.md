# 要件定義: ツール (REQ-TOOL)

## スコープ概要

Valen の開発ツールチェーンに関する要件。コンパイラ CLI（valenc）、Gradle プラグイン、LSP サーバー、コードフォーマッタ（valenfmt）を定義する。

**関連仕様:** [IMPLEMENTATION_PLAN.md](../IMPLEMENTATION_PLAN.md) Phase 1 ツールセクション
**Phase:** MVP（Phase 1）/ Phase 1.5

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス |
|----|---------|--------|-----------|
| REQ-TOOL-001 | valenc CLI（build / check / version サブコマンド） | Must | Draft |
| REQ-TOOL-002 | Gradle プラグイン（compileValen タスク、標準 sourceSet） | Must | Draft |
| REQ-TOOL-003 | LSP サーバー（syntax error・型診断・goto definition） | Must | Draft |
| REQ-TOOL-004 | valenfmt 最小版（brace style・indent・trailing semicolon） | Should | Draft |

---

## REQ-TOOL-001: valenc CLI（build / check / version サブコマンド）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TOOL-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

Valen コンパイラの CLI エントリポイント。Rust で実装し、以下のサブコマンドを提供する。

| サブコマンド | 機能 |
|-------------|------|
| `valenc build <source>` | `.vln` ソースをコンパイルし .class ファイルを出力 |
| `valenc check <source>` | 型チェック・lint のみ実行（.class 生成なし） |
| `valenc version` | バージョン情報を表示 |

- 出力ディレクトリは `--output` / `-o` フラグで指定（デフォルト: `./build/classes`）
- ターゲット JVM バージョンは `--target` フラグで指定（デフォルト: 21）
- エラー出力は stderr、診断メッセージは構造化フォーマット（行:列、エラーコード）
- 終了コード: 0=成功、1=コンパイルエラー、2=CLI 引数エラー

### 受入条件

- [ ] `valenc build hello.vln` で .class ファイルが生成される
- [ ] `valenc check hello.vln` で型チェックが実行され、.class が生成されない
- [ ] `valenc version` でバージョン文字列が表示される
- [ ] `--output` フラグで出力ディレクトリを変更できる
- [ ] `--target` フラグで JVM ターゲットバージョンを変更できる
- [ ] コンパイルエラー時に行番号・列番号・エラーコードを含む診断メッセージが stderr に出力される
- [ ] 不正な引数で終了コード 2 が返される
- [ ] 複数ファイルの一括コンパイルが可能

### 依存

- REQ-EMIT-001（Java 21 class file 生成）

---

## REQ-TOOL-002: Gradle プラグイン（compileValen タスク、標準 sourceSet）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TOOL-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

Kotlin で実装する Gradle プラグイン。Valen ソースコードの Gradle ビルドシステムへの統合を提供する。

**プラグイン ID:** `com.valen.lang`（仮）

**提供するタスク:**

| タスク | 機能 |
|--------|------|
| `compileValen` | `src/main/valen/*.vln` をコンパイルし .class を出力 |
| `compileTestValen` | `src/test/valen/*.vln` をコンパイル |

**sourceSet 対応:**
- `src/main/valen/` — メインソース
- `src/test/valen/` — テストソース

**設定 DSL:**
```kotlin
valen {
    target = 21        // JVM ターゲット（デフォルト: 21）
    valencPath = "..."  // valenc バイナリパス（自動検出あり）
}
```

- Java プラグインとの共存（Java + Valen 混合プロジェクト）
- 依存関係解決は Gradle 標準の仕組みに従属
- incremental compilation 対応（ファイル単位の変更検出）

### 受入条件

- [ ] `./gradlew compileValen` で `src/main/valen/*.vln` がコンパイルされる
- [ ] 生成された .class が `build/classes/valen/main/` に出力される
- [ ] Java プラグインと共存し、Java ソースから Valen の .class を参照可能
- [ ] Valen ソースから Java の .class を参照可能（import 解決）
- [ ] sourceSet が標準の `src/main/valen/` と `src/test/valen/` に対応
- [ ] `valen {}` DSL でターゲットバージョンと valenc パスを設定可能
- [ ] ソース変更時に incremental compilation が動作する
- [ ] コンパイルエラーが Gradle コンソールに構造化表示される

### 依存

- REQ-TOOL-001（valenc CLI — Gradle から呼び出す対象）
- REQ-INTEROP-001（Java クラスの import — 混合プロジェクト前提）

---

## REQ-TOOL-003: LSP サーバー（syntax error・型診断・goto definition）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TOOL-003 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

Rust で実装する Language Server Protocol サーバー。MVP では以下の機能を提供する。

| LSP 機能 | 対応 |
|----------|------|
| `textDocument/publishDiagnostics` | syntax error、型エラーの診断 |
| `textDocument/definition` | goto definition（関数・型・変数） |
| `textDocument/didOpen` | ファイルオープン時の初期診断 |
| `textDocument/didChange` | リアルタイム再解析・再診断 |

- LSP 3.17 以降に準拠
- VSCode 拡張を同梱（MVP の動作確認用）
- incremental parsing を前提に設計（salsa crate 等の検討含む）
- メモリ使用量とレスポンス時間に留意（大規模プロジェクトでの劣化を抑制）

### 受入条件

- [ ] VSCode で `.vln` ファイルを開くと LSP が起動する
- [ ] syntax error がリアルタイムで赤波線表示される
- [ ] 型エラーが診断として表示される
- [ ] 関数名・型名・変数名に対して goto definition が動作する
- [ ] ファイル保存時だけでなく、入力中にリアルタイム診断が更新される
- [ ] 診断メッセージにエラーコード・行番号・列番号が含まれる
- [ ] 複数ファイルプロジェクトで cross-file の定義ジャンプが動作する
- [ ] LSP サーバーがクラッシュした場合に自動再起動される

### 依存

- REQ-TOOL-001（valenc — パーサー・型チェッカーの共有）

---

## REQ-TOOL-004: valenfmt 最小版（brace style・indent・trailing semicolon）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TOOL-004 |
| **優先度** | Should |
| **ステータス** | Draft |
| **Phase** | MVP / Phase 1.5 拡張 |

### 説明

Rust で実装するコードフォーマッタの最小版。MVP では以下の整形ルールのみを対象とする。

| ルール | 内容 |
|--------|------|
| brace style | K&R スタイル（開き中括弧は同一行） |
| indent | スペース 4 つ（固定） |
| trailing semicolon | 不要なセミコロンの除去 |

- `valenfmt <file>` でファイルを整形（in-place）
- `valenfmt --check <file>` で差分検出のみ（CI 用）
- stdin/stdout パイプ対応
- Phase 1.5 以降で追加ルール（改行、空行、import 順序等）を拡張

### 受入条件

- [ ] `valenfmt hello.vln` でファイルが整形される
- [ ] brace style が K&R に統一される
- [ ] インデントがスペース 4 つに統一される
- [ ] 不要な trailing semicolon が除去される
- [ ] `--check` フラグで差分がある場合に非ゼロ終了コードが返される
- [ ] stdin からの入力を受け付け、stdout に整形結果を出力できる
- [ ] 整形がコードの意味を変えないことが保証される
- [ ] コメントが保持される

### 依存

- REQ-SYNTAX-001（字句定義 — パーサーの共有）

---

## 変更履歴

| 日付 | 変更内容 | 担当 |
|------|---------|------|
| 2026-05-11 | 初版作成 | — |
