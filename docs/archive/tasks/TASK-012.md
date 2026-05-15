## TASK-012: 標準ライブラリ (valen.core)

| 項目 | 内容 |
|------|------|
| ID | TASK-012 |
| 関連要件 | REQ-STDLIB-001, REQ-STDLIB-002, REQ-STDLIB-003, REQ-FAIL-002 |
| 規模 | M |
| 依存タスク | TASK-010 |

### 実装概要
Valen で標準ライブラリを実装する。Option<T>、Result<T,E>、Error trait、Iterator trait、基本コレクションの typealias + trait injection、基本 IO ラッパーを含む。

### 対象ファイル
- `stdlib/` ディレクトリ（新規 .vln ファイル群）

### 実装ステップ
1. `Option<T>` enum（Some/None）を定義し map/unwrap/? サポートを実装
2. `Result<T,E>` enum（Ok/Err）を定義し map/map_err/? サポートを実装
3. `Error` trait を定義
4. `Iterator<T>` trait を定義し `next()` メソッドを実装
5. コレクション typealias を定義（List/Map/Set → java.util）
6. Iterator に map/filter を実装
7. 基本 IO ラッパー（valen.io）を実装
8. テスト追加
