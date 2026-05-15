# 要件定義: 失敗モデル (REQ-FAIL)

## スコープ概要

Valen の失敗モデルに関する要件。Option / Result / panic / Exception の役割分離、Error trait、`?` 演算子、`safe {}` ブロックによる Java 例外変換を定義する。

**関連仕様:** [lang/08-failure.md](../lang/08-failure.md)
**Phase:** MVP（Phase 1）

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス |
|----|---------|--------|-----------|
| REQ-FAIL-001 | Option/Result/panic/Exception の役割分離 | Must | Draft |
| REQ-FAIL-002 | Error trait（valen.core） | Must | Draft |
| REQ-FAIL-003 | `?` 演算子（同一 E 型のみ伝播） | Must | Draft |
| REQ-FAIL-004 | `safe {}` ブロック（Java 例外 → Result） | Must | Draft |
| REQ-FAIL-005 | `safe {}` 内 Java 戻り値は T?（Option\<T\>） | Must | Draft |

---

## REQ-FAIL-001: Option/Result/panic/Exception の役割分離

| 項目 | 内容 |
|------|------|
| **ID** | REQ-FAIL-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

Valen の失敗モデルは以下の4層に厳格に分離する。各層の責務が重複してはならない。

| 層 | 用途 | 型/構文 |
|----|------|---------|
| Option | 値の不在 | `Option<T>` / `T?` 糖衣 |
| Result | 回復可能なエラー | `Result<T, E: Error>` |
| panic | 契約違反（プログラムバグ） | `panic("message")` |
| Exception | Java FFI 専用 | `safe {}` ブロック内でのみ出現 |

- Option を「エラー」として使用してはならない。意味的に「値が無い」場合にのみ使用
- Result を「値の不在」に使用してはならない。回復可能なエラーにのみ使用
- panic はプログラムの論理的バグ（事前条件違反、不変条件破壊）にのみ使用。回復は想定しない
- Exception は Valen コード内で直接 throw/catch しない。`safe {}` ブロック経由で Result に変換する

### 受入条件

- [ ] Option\<T\> は「値の不在」を表現し、エラー情報を持たない
- [ ] Result\<T, E\> は「回復可能なエラー」を表現し、E に Error trait 制約がある
- [ ] panic は即座にプロセスを停止し、catch 不可
- [ ] Exception は `safe {}` ブロック外では直接扱えない
- [ ] コンパイラが上記の役割境界違反を検出しエラーを報告する
- [ ] 言語仕様ドキュメントに4層の判断フローチャートが記載されている

### 依存

- REQ-TYPE-005（Option\<T\> による null 一本化）
- REQ-STDLIB-001（valen.core の Option/Result 定義）

---

## REQ-FAIL-002: Error trait（valen.core）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-FAIL-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`valen.core` に `Error` trait を定義する。Result の型パラメータ E は `E: Error` 制約を持つ。

```valen
trait Error {
    fn message(self) -> String;
}
```

- `Result<T, E>` の E は必ず `Error` を impl していなければならない
- ユーザ定義のエラー型は `impl Error for MyError` で Error trait を満たす
- `JavaException`（`safe {}` 用の内部型）も Error を impl する

### 受入条件

- [ ] `valen.core.Error` trait が定義され、`fn message(self) -> String` メソッドを持つ
- [ ] `Result<T, E>` の E に `E: Error` 制約がコンパイル時に強制される
- [ ] Error を impl していない型を Result の E に使用するとコンパイルエラー
- [ ] ユーザ定義型に対して `impl Error for T` が記述可能
- [ ] `JavaException` が Error trait を impl している
- [ ] Error trait の orphan rule が正しく適用される

### 依存

- REQ-TRAIT-001（trait 定義と impl ブロック）
- REQ-TRAIT-003（orphan rule）
- REQ-STDLIB-001（valen.core パッケージ）

---

## REQ-FAIL-003: `?` 演算子（同一 E 型のみ伝播）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-FAIL-003 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`?` 演算子は Result または Option の早期リターンを提供する。以下の制約を厳格に適用する。

**Result に対する `?`:**
- `Result<T, E>` に適用すると、Err(e) の場合は即座に `return Err(e)` する
- 呼び出し元関数の戻り値型が `Result<_, E>` であり、E が同一型でなければコンパイルエラー
- 異なるエラー型間の暗黙変換（From trait 等）は MVP では導入しない

**Option に対する `?`:**
- `Option<T>` に適用すると、None の場合は即座に `return None` する
- 呼び出し元関数の戻り値型が `Option<_>` でなければコンパイルエラー

**禁止:**
- Option → Result への暗黙変換（`Option?` を Result 返却関数内で使用不可）
- Result → Option への暗黙変換

### 受入条件

- [ ] `Result<T, E>` に `?` を適用し、Err 時に早期リターンが動作する
- [ ] `Option<T>` に `?` を適用し、None 時に早期リターンが動作する
- [ ] 呼び出し元の戻り値型が `Result` でない関数内で Result に `?` を使用するとコンパイルエラー
- [ ] 呼び出し元の戻り値型が `Option` でない関数内で Option に `?` を使用するとコンパイルエラー
- [ ] Result の E 型が呼び出し元と一致しない場合にコンパイルエラー
- [ ] Option に `?` を適用した結果が Result に変換されないことを確認
- [ ] Result に `?` を適用した結果が Option に変換されないことを確認
- [ ] エラーメッセージが型の不一致を明確に報告する

### 依存

- REQ-FAIL-001（役割分離の原則）
- REQ-FAIL-002（Error trait）
- REQ-TYPE-006（ジェネリクス）

---

## REQ-FAIL-004: `safe {}` ブロック（Java 例外 → Result）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-FAIL-004 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`safe {}` ブロックは Java FFI 境界を明示する構文。ブロック内で発生した Java の checked/unchecked exception を `Result<T, JavaException>` に変換する。

```valen
let result: Result<String, JavaException> = safe {
    SomeJavaClass.riskyMethod()
};
```

- `safe {}` ブロックの型は `Result<T, JavaException>` となる
- ブロック内では Java メソッド呼び出しが可能
- Java メソッドが例外を throw した場合、`Err(JavaException(...))` に変換される
- 例外が発生しなかった場合、`Ok(value)` に変換される
- `safe {}` ブロック外で Java の例外を直接 catch する手段は提供しない

### 受入条件

- [ ] `safe {}` ブロックの戻り値型が `Result<T, JavaException>` である
- [ ] Java メソッドが checked exception を throw した場合に `Err(JavaException)` に変換される
- [ ] Java メソッドが unchecked exception を throw した場合に `Err(JavaException)` に変換される
- [ ] 例外が発生しない場合に `Ok(value)` が返される
- [ ] `safe {}` ブロック外で try-catch 相当の構文が存在しないことを確認
- [ ] `JavaException` 型が Error trait を impl している
- [ ] `JavaException` から元の Java 例外情報（型名・メッセージ・スタックトレース）にアクセス可能
- [ ] ネストした `safe {}` ブロックが正しく動作する

### 依存

- REQ-FAIL-001（役割分離 — Exception は FFI 専用）
- REQ-FAIL-002（Error trait — JavaException が impl）
- REQ-INTEROP-001（Java クラスの import と利用）

---

## REQ-FAIL-005: `safe {}` 内 Java 戻り値は T?（Option\<T\>）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-FAIL-005 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`safe {}` ブロック内で呼び出した Java メソッドの戻り値は、参照型の場合 `T?`（`Option<T>`）として扱う。Java の null を安全に Valen の型システムに変換するための規則。

- Java メソッドの戻り値が参照型の場合: `T?`（`Option<T>`）として型付けされる
- Java メソッドの戻り値が void の場合: `Unit` に変換される
- Java メソッドの戻り値がプリミティブ型の場合: 対応する Valen プリミティブ型（非 Option）

これにより、`safe {}` ブロック全体の型は:
- 正常時: `Ok(T?)` または `Ok(Unit)`
- 例外時: `Err(JavaException)`

### 受入条件

- [ ] `safe {}` 内の Java メソッド戻り値（参照型）が `T?` として型付けされる
- [ ] Java メソッドが null を返した場合に `None` に変換される
- [ ] Java メソッドが非 null を返した場合に `Some(value)` に変換される
- [ ] Java void メソッドの結果が `Unit` に変換される
- [ ] Java プリミティブ型の戻り値は Option でラップされない
- [ ] `safe {}` 全体の型が `Result<T?, JavaException>` または `Result<Unit, JavaException>` となる
- [ ] 型推論が `safe {}` 内の Java 戻り値に対して正しく動作する

### 依存

- REQ-FAIL-004（safe {} ブロック）
- REQ-TYPE-005（Option\<T\> による null 一本化）
- REQ-INTEROP-001（Java クラスの import と利用）

---

## 変更履歴

| 日付 | 変更内容 | 担当 |
|------|---------|------|
| 2026-05-11 | 初版作成 | — |
