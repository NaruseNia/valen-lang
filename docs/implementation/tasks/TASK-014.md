## TASK-014: LSP サーバー (MVP)

| 項目 | 内容 |
|------|------|
| ID | TASK-014 |
| 関連要件 | REQ-TOOL-003 |
| 規模 | M |
| 依存タスク | TASK-004, TASK-005 |

### 実装概要
tower-lsp を使用した LSP サーバーを実装し、リアルタイム構文エラー、型診断、定義ジャンプを提供する。

### 対象ファイル
- `crates/valen-lsp/src/main.rs`

### 実装ステップ
1. textDocument/didOpen および didChange ハンドラを実装
2. ドキュメント変更時にパーサーを実行し diagnostics を publish
3. HIR 名前解決を実行し型エラーを検出
4. textDocument/definition（定義ジャンプ）を実装
5. ドキュメント変更 → インクリメンタル再パースの接続
6. LSP プロトコルによるテスト追加
