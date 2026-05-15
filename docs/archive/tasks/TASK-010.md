## TASK-010: Codegen: fn/method/expression バイトコード生成

| 項目 | 内容 |
|------|------|
| ID | TASK-010 |
| 関連要件 | REQ-TOOL-001, REQ-EMIT-004, REQ-TYPE-004, REQ-FAIL-003 |
| 規模 | L |
| 依存タスク | TASK-005 |

### 実装概要
関数本体のコアバイトコード生成を実装する。ローカル変数、式（算術・比較・論理）、制御フロー（if/match/for/while/loop/break/continue）、メソッドディスパッチ、`?` 演算子のローワリング、`==` → `equals()` デシュガーを含む。

### 対象ファイル
- `crates/valen-codegen/src/lib.rs`
- `crates/valen-codegen/src/expr_emit.rs`（新規作成）
- `crates/valen-codegen/src/fn_emit.rs`（新規作成）

### 実装ステップ
1. ローカル変数スロットとスタックフレームの emit
2. 算術・比較・論理演算の emit
3. if/else を分岐命令として emit
4. match を tableswitch/lookupswitch + instanceof チェーンとして emit
5. for/while/loop を break/continue ラベル付きで emit
6. メソッド呼び出しの emit（invokevirtual/invokeinterface/invokestatic）
7. `?` 演算子の emit（Ok/Err または Some/None での分岐）
8. `==` を invokevirtual equals、`===` を if_acmpeq として emit
9. 文字列補間の emit（StringBuilder パターン）
10. valenc ビルドのエンドツーエンド結合
11. テスト追加
