## TASK-006: Coherence / orphan rule

| 項目 | 内容 |
|------|------|
| ID | TASK-006 |
| 関連要件 | REQ-TRAIT-003 |
| 規模 | M |
| 依存タスク | TASK-004 |

### 実装概要
impl ブロックに対する orphan rule の検証、グローバル一意性チェック、blanket impl の拒否を実装する。

### 対象ファイル
- `crates/valen-hir/src/coherence.rs`

### 実装ステップ
1. モジュール単位での impl の trait/type 所有権チェック
2. foreign-trait-for-foreign-type の拒否
3. blanket impl の拒否
4. グローバル impl 一意性チェック
5. typealias による所有権バイパスの拒否
6. テスト追加
