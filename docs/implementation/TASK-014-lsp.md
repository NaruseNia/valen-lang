# TASK-014: Valen LSP サーバー（MVP）実装計画

## 概要

Valen 用 Language Server Protocol サーバーの MVP 実装。async-lsp フレームワークを使用。
将来的に `NaruseNia/valen-lsp` 別リポに分離予定。

## 技術選定

| 項目 | 選定 | 理由 |
|------|------|------|
| フレームワーク | **async-lsp** (0.2) | tower Service 統合、omnitrait API、nil で実稼働実績 |
| ~~tower-lsp~~ | 不採用 | 3年放置、メンテ停止 |
| 非同期ランタイム | tokio | async-lsp の transport で使用 |
| LSP型定義 | lsp-types | async-lsp が re-export |

## MVP 機能

| 機能 | LSP メソッド | 説明 |
|------|-------------|------|
| 診断表示 | `textDocument/publishDiagnostics` | parse + resolve + type_check エラーをリアルタイム表示 |
| 定義ジャンプ | `textDocument/definition` | 関数・型・変数の定義元 Span にジャンプ |

## Phase 1.5 送り

- completion / hover / semantic tokens
- cross-file 解決（ワークスペース）
- UTF-16 offset 対応
- インクリメンタル解析（salsa 等）
- debouncing

## アーキテクチャ

```
crates/valen-lsp/src/
├── main.rs       — tokio entrypoint + async-lsp stdio transport
├── server.rs     — ServerState + LanguageServer omnitrait impl
├── analysis.rs   — DocumentState, analyze_document (parse→resolve→typecheck)
├── convert.rs    — LineIndex, Span→LSP Position, Diagnostic 変換
└── goto_def.rs   — カーソル位置→HIR def 検索
```

### 状態管理

```rust
struct ServerState {
    client: ClientSocket,
    documents: HashMap<Url, DocumentState>,
}

struct DocumentState {
    text: String,
    line_index: LineIndex,
    items: Vec<valen_ast::Item>,
    hir: Option<valen_hir::Hir>,
    version: i32,
}
```

- `&mut self` で notification 処理（async-lsp の設計に沿う）
- 変更のたびにフルre-parse（MVP ではインクリメンタル不要）
- 単一ファイル解析（cross-file は Phase 1.5）

### async-lsp 統合パターン

omnitrait `LanguageServer` を ServerState に impl:

```rust
impl LanguageServer for ServerState {
    // requests: &self → ResponseFuture（並行実行可能）
    fn initialize(&self, params: InitializeParams) -> ResponseFuture<InitializeResult> { ... }
    fn definition(&self, params: GotoDefinitionParams) -> ResponseFuture<Option<...>> { ... }

    // notifications: &mut self（逐次実行）
    fn did_open(&mut self, params: DidOpenTextDocumentParams) { ... }
    fn did_change(&mut self, params: DidChangeTextDocumentParams) { ... }
    fn did_close(&mut self, params: DidCloseTextDocumentParams) { ... }
}
```

### 解析パイプライン

```
did_open / did_change
  → parse(source, FileId(0))
  → resolve(&items)
  → type_check(&hir, &items)
  → collect diagnostics
  → client.publish_diagnostics(uri, diags)
```

### goto-definition

1. カーソル byte offset を LineIndex で算出
2. source text からカーソル位置の識別子を抽出
3. HIR defs テーブルで名前検索
4. def.span を LSP Location に変換して返却

## 依存関係変更

### 削除
- `tower-lsp` (workspace + crate)

### 追加
- `async-lsp = { version = "0.2", features = ["omni"] }` (workspace)
- `lsp-types` (async-lsp が re-export するが、直接使用するなら追加)
- `futures` (async-lsp の transport に必要)

## コミット計画

| # | 内容 | 規模 |
|---|------|------|
| 1 | tower-lsp → async-lsp 依存差し替え + scaffold | S |
| 2 | convert.rs: LineIndex + Diagnostic 変換 | S |
| 3 | analysis.rs: DocumentState + 解析パイプライン | M |
| 4 | server.rs: did_open/did_change/did_close + 診断公開 | M |
| 5 | goto_def.rs: 定義ジャンプ | M |
| 6 | テスト + ドキュメント | S |

## 受入条件

- [ ] .vln ファイルを開くと parse/type エラーが diagnostics として表示される
- [ ] エラー箇所の行・列が正確
- [ ] 関数名・型名で goto definition が動作する
- [ ] `valenc build` の診断コード (V0xxx) が LSP diagnostic code に反映される
- [ ] VS Code + 汎用 LSP クライアントで動作確認
