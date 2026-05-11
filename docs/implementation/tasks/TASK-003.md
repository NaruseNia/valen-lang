## TASK-003: Parser 拡張（for/while/loop/break/continue/lambda）

| 項目 | 内容 |
|------|------|
| ID | TASK-003 |
| 関連要件 | REQ-SYNTAX-002 |
| 規模 | M |
| 依存タスク | - |

### 実装概要
ループ構文（for-in、while、loop）、break（値付き）、continue、ラムダ式のパースを実装する。

### 対象ファイル
- `crates/valen-parser/src/parser.rs`

### 実装ステップ
1. for-in 式パース追加
2. while 式パース追加
3. loop 式パース追加
4. break / break expr / continue パース追加
5. ラムダ式パース追加（`|params| body` 形式）
6. return 式パース追加
7. スナップショットテスト追加
