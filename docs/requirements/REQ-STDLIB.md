# 要件定義: 標準ライブラリ (REQ-STDLIB)

## スコープ概要

Valen 標準ライブラリに関する要件。`valen.core`（Option / Result / Error / Iterator）、`valen.collections`（java.util typealias + trait 注入）、`valen.io`（基本 IO ラッパー）を定義する。

**関連仕様:** [IMPLEMENTATION_PLAN.md](../IMPLEMENTATION_PLAN.md) Phase 1 標準ライブラリセクション
**Phase:** MVP（Phase 1）/ Phase 1.5

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス |
|----|---------|--------|-----------|
| REQ-STDLIB-001 | valen.core（Option, Result, Error trait, Iterator trait） | Must | Draft |
| REQ-STDLIB-002 | valen.collections（List/Map/Set = java.util typealias + trait 注入） | Must | Draft |
| REQ-STDLIB-003 | valen.io（基本 IO ラッパー、safe ブロック IOException → Result） | Should | Draft |

---

## REQ-STDLIB-001: valen.core（Option, Result, Error trait, Iterator trait）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-STDLIB-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`valen.core` パッケージは Valen の基盤となる型と trait を提供する。暗黙的にすべての Valen ソースファイルに import される（prelude）。

**提供する型:**

| 型 / trait | 定義 |
|------------|------|
| `Option<T>` | `enum Option<T> { Some(T), None }` — 値の有無を表現 |
| `Result<T, E: Error>` | `enum Result<T, E: Error> { Ok(T), Err(E) }` — 回復可能エラー |
| `Error` | `trait Error { fn message(self) -> String; }` — エラー型の共通インターフェース |
| `Iterator<T>` | `trait Iterator<T> { fn next(mut self) -> Option<T>; }` — イテレーション |

**Option の糖衣構文:**
- `T?` は `Option<T>` の糖衣
- `None` は `Option::None` の省略形

**prelude:**
- `valen.core` の全 public 型・trait は明示 import なしで使用可能
- ユーザが同名の型を定義した場合は shadowing（明示 import で解決）

### 受入条件

- [ ] `Option<T>` が `Some(T)` と `None` の2 variant を持つ enum として定義される
- [ ] `Result<T, E: Error>` が `Ok(T)` と `Err(E)` の2 variant を持つ enum として定義される
- [ ] `T?` 糖衣が `Option<T>` に展開される
- [ ] `Error` trait が `fn message(self) -> String` メソッドを持つ
- [ ] `Iterator<T>` trait が `fn next(mut self) -> Option<T>` メソッドを持つ
- [ ] `for x in iter` 構文が Iterator trait に対して動作する
- [ ] `java.lang.Iterable` が自動的に Iterator にアダプトされる
- [ ] `valen.core` の型が明示 import なしで使用可能（prelude）
- [ ] Option/Result に対する `?` 演算子が動作する（REQ-FAIL-003）
- [ ] Option/Result に対する match の exhaustive check が動作する

### 依存

- REQ-ADT-001（enum 定義 — Option/Result は enum で実装）
- REQ-TRAIT-001（trait 定義 — Error/Iterator は trait）
- REQ-TYPE-005（Option\<T\> による null 一本化）
- REQ-TYPE-006（ジェネリクス — 型パラメータ T, E）

---

## REQ-STDLIB-002: valen.collections（List/Map/Set = java.util typealias + trait 注入）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-STDLIB-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`valen.collections` パッケージは Java の標準コレクションに Valen の typealias と trait 注入を提供する。独自コレクション実装は行わず、`java.util` に依存する。

**typealias:**

| Valen 型 | Java 型 |
|-----------|---------|
| `List<T>` | `java.util.List<T>` |
| `Map<K, V>` | `java.util.Map<K, V>` |
| `Set<T>` | `java.util.Set<T>` |

**trait 注入（MVP）:**

Iterator trait の impl により、以下のメソッドを typealias 経由で利用可能にする。

| メソッド | シグネチャ | 機能 |
|---------|----------|------|
| `map` | `fn map<U>(self, f: (T) -> U) -> List<U>` | 変換 |
| `filter` | `fn filter(self, f: (T) -> Bool) -> List<T>` | フィルタ |

- `java.util.List` → `List<T>` として import し、`.map()` / `.filter()` が UFCS で呼び出せる
- `for x in list` が動作する（`java.lang.Iterable` → Iterator アダプト経由）
- Phase 1.5 で `reduce` / `fold` / `groupBy` 等を追加予定

### 受入条件

- [ ] `List<T>` が `java.util.List<T>` の typealias として定義される
- [ ] `Map<K, V>` が `java.util.Map<K, V>` の typealias として定義される
- [ ] `Set<T>` が `java.util.Set<T>` の typealias として定義される
- [ ] `List<T>` に対して `.map(f)` が呼び出せる
- [ ] `List<T>` に対して `.filter(f)` が呼び出せる
- [ ] `for x in list` が動作する
- [ ] `Map` / `Set` に対して `for` ループが動作する
- [ ] Java の `ArrayList` / `HashMap` / `HashSet` が typealias 経由で利用可能
- [ ] trait 注入が orphan rule に違反しないことを確認（trait は自 module 所有）
- [ ] `java.util.List` を直接 import した場合と typealias 経由の場合で互換性がある

### 依存

- REQ-TYPE-007（typealias）
- REQ-TRAIT-001（trait 定義と impl — map/filter の注入）
- REQ-TRAIT-003（orphan rule — java.util 型への trait impl が合法か確認）
- REQ-STDLIB-001（Iterator trait — for ループの基盤）
- REQ-INTEROP-001（Java クラスの import — java.util への依存）

---

## REQ-STDLIB-003: valen.io（基本 IO ラッパー、safe ブロック IOException → Result）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-STDLIB-003 |
| **優先度** | Should |
| **ステータス** | Draft |
| **Phase** | MVP / Phase 1.5 拡張 |

### 説明

`valen.io` パッケージは基本的なファイル IO 機能を提供する。Java の `java.io` / `java.nio` を `safe {}` ブロックで包み、IOException を `Result` に変換した安全な API を提供する。

**提供する関数（MVP）:**

| 関数 | シグネチャ | 機能 |
|------|----------|------|
| `read_string` | `fn read_string(path: String) -> Result<String, IoError>` | ファイル全体を文字列として読み込み |
| `write_string` | `fn write_string(path: String, content: String) -> Result<Unit, IoError>` | 文字列をファイルに書き出し |
| `read_lines` | `fn read_lines(path: String) -> Result<List<String>, IoError>` | ファイルを行単位で読み込み |

**IoError:**

```valen
class IoError(pub message: String, pub cause: Option<JavaException>)
impl Error for IoError {
    fn message(self) -> String { self.message }
}
```

- `IoError` は `Error` trait を impl する
- Java の `IOException` を内部で `safe {}` により捕捉し、`IoError` に変換
- Phase 1.5 以降でバッファリング IO、ストリーム、パス操作等を拡張予定

### 受入条件

- [ ] `valen.io.read_string(path)` でファイル内容を `Result<String, IoError>` として読み込める
- [ ] `valen.io.write_string(path, content)` でファイルに書き出しが `Result<Unit, IoError>` で行える
- [ ] `valen.io.read_lines(path)` でファイルを行単位で `Result<List<String>, IoError>` として読み込める
- [ ] 存在しないファイルを読み込もうとすると `Err(IoError)` が返される
- [ ] 書き込み権限がないパスに書き出そうとすると `Err(IoError)` が返される
- [ ] `IoError` が Error trait を impl している
- [ ] `IoError` から元の Java 例外情報（`cause`）にアクセス可能
- [ ] `?` 演算子で IoError を伝播できる
- [ ] 内部実装が `safe {}` ブロックを使用して Java IO を呼び出している

### 依存

- REQ-FAIL-002（Error trait — IoError が impl）
- REQ-FAIL-004（safe {} ブロック — 内部実装で使用）
- REQ-INTEROP-001（Java クラスの import — java.io への依存）
- REQ-STDLIB-001（valen.core — Result/Option の基盤）
- REQ-STDLIB-002（valen.collections — List\<String\> の利用）

---

## 変更履歴

| 日付 | 変更内容 | 担当 |
|------|---------|------|
| 2026-05-11 | 初版作成 | — |
