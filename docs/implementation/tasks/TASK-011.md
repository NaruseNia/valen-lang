## TASK-011: Codegen: Java 相互運用

| 項目 | 内容 |
|------|------|
| ID | TASK-011 |
| 関連要件 | REQ-INTEROP-001, REQ-INTEROP-002, REQ-INTEROP-003, REQ-FAIL-004, REQ-FAIL-005 |
| 規模 | L |
| 依存タスク | TASK-005 |

### 実装概要
クラスパスからの Java クラス解決、`safe {}` ブロックのローワリング（try-catch → Result ラッピング）、null → Option 変換、`@valen.Closed` アノテーション読み取りを実装する。

### 対象ファイル
- `crates/valen-codegen/src/interop.rs`（新規作成）
- `crates/valen-hir/src/resolve.rs`

### 実装ステップ
1. クラスパスから Java .class ファイルを読み取り型解決
2. Java 型を Valen 外部型にマッピング
3. `safe {}` ブロックを try-catch バイトコードとして生成し Result にラッピング
4. Java 戻り値に null チェックを挿入し Option にラッピング
5. Java sealed 型から `@valen.Closed` アノテーションを読み取り
6. closed アノテーション情報を網羅性チェッカーに供給
7. 実際の Java クラスファイルを使ったテスト追加
