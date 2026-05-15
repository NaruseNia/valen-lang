## TASK-005: 型検査

| 項目 | 内容 |
|------|------|
| ID | TASK-005 |
| 関連要件 | REQ-TYPE-001, REQ-TYPE-002, REQ-TYPE-003, REQ-TYPE-004, REQ-TYPE-005, REQ-TYPE-006, REQ-TYPE-007, REQ-TYPE-008, REQ-FAIL-001, REQ-FAIL-003, REQ-CLASS-003 |
| 規模 | L |
| 依存タスク | TASK-004 |

### 実装概要
型検査、ローカル変数の型推論、ジェネリクスインスタンス化、`?` 演算子の型規則、暗黙的数値変換の拒否を実装する。

### 対象ファイル
- `crates/valen-hir/src/ty.rs`
- `crates/valen-hir/src/typeck.rs`（新規作成）

### 実装ステップ
1. 型表現の定義（プリミティブ、ジェネリクス、関数型、Option/Result）
2. ローカル型推論の実装（双方向型推論）
3. 関数シグネチャ型の明示宣言チェック
4. ジェネリクスインスタンス化（variance 付き）のチェック
5. `?` 演算子の検証（同一 E 型、Option は Option 関数内のみ）
6. 暗黙的数値変換の拒否
7. `==` が `.equals()` に、`===` が参照比較にデシュガーされることの検査
8. テスト追加
