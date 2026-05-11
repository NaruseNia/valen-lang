## TASK-004: HIR 設計と名前解決

| 項目 | 内容 |
|------|------|
| ID | TASK-004 |
| 関連要件 | REQ-CLASS-005, REQ-CLASS-006, REQ-CLASS-007, REQ-CLASS-008, REQ-TRAIT-001, REQ-TRAIT-002, REQ-TRAIT-004, REQ-TYPE-007 |
| 規模 | L |
| 依存タスク | TASK-001, TASK-002 |

### 実装概要
HIR（High-level Intermediate Representation）のデータ構造を設計し、名前解決（import、パス、可視性）、スコープ管理、メソッド解決順序を実装する。

### 対象ファイル
- `crates/valen-hir/src/lib.rs`
- `crates/valen-hir/src/resolve.rs`

### 実装ステップ
1. HIR ノード型設計（HirItem, HirExpr 等）
2. シンボルテーブル / スコープチェーン構築
3. import 宣言の解決
4. 型パスの解決
5. 値パスの解決（変数、関数）
6. メソッド解決の実装（class body > trait > UFCS エラー）
7. 可視性制約の検証
8. テスト追加
