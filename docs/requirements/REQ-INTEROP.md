# 要件定義: Java 相互運用 (REQ-INTEROP)

## スコープ概要

Valen と Java の相互運用（interop）に関する要件。Java クラスの import・利用、`safe {}` による例外変換、`@valen.Closed` アノテーションによる Java sealed 型の exhaustive match を定義する。

**関連仕様:** [LANGUAGE_SPEC.md](../LANGUAGE_SPEC.md) / [lang/08-failure.md](../lang/08-failure.md)
**Phase:** MVP（Phase 1）

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス |
|----|---------|--------|-----------|
| REQ-INTEROP-001 | Java クラスの import と利用 | Must | Draft |
| REQ-INTEROP-002 | `safe {}` による例外変換（Java exception → Result） | Must | Draft |
| REQ-INTEROP-003 | `@valen.Closed` で Java sealed 型を exhaustive match | Must | Draft |

---

## REQ-INTEROP-001: Java クラスの import と利用

| 項目 | 内容 |
|------|------|
| **ID** | REQ-INTEROP-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

Valen コードから Java クラス・インターフェースを import し利用する機能。

**import 構文:**

```valen
import java.util.ArrayList;
import java.io.File as JavaFile;  // alias
```

- 単一型 import のみ（ワイルドカード `import java.util.*` は不可）
- `as` による alias 付与が可能
- import した Java 型は「foreign 型」として扱われる
- foreign 型に対する trait impl は orphan rule の対象（Java 型は自 module 所有ではない → trait 側が自 module 所有でなければ impl 不可）
- Java のメソッド呼び出し、フィールドアクセス、コンストラクタ呼び出しが可能
- Java の static メソッド・フィールドへのアクセスが可能
- overload 解決は Java の規則に準拠

### 受入条件

- [ ] `import java.util.ArrayList;` で Java クラスが利用可能になる
- [ ] `import path.to.Type as Alias;` で alias が設定される
- [ ] ワイルドカード import がコンパイルエラーになる
- [ ] import した Java クラスのメソッド呼び出しが動作する
- [ ] import した Java クラスのコンストラクタ呼び出しが動作する
- [ ] import した Java クラスの static メソッド・フィールドにアクセスできる
- [ ] foreign 型に対する orphan rule が正しく適用される
- [ ] Java メソッドの overload 解決が動作する
- [ ] 存在しない Java クラスの import でコンパイルエラーが報告される
- [ ] classpath 上の Java クラスのみが import 対象となる

### 依存

- REQ-CLASS-008（import 構文の定義）

---

## REQ-INTEROP-002: `safe {}` による例外変換（Java exception → Result）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-INTEROP-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

`safe {}` ブロックは Java FFI 境界を明示する構文であり、Java コード呼び出し時の例外を `Result<T, JavaException>` に変換する。REQ-FAIL-004 で定義された失敗モデルの interop 側要件。

```valen
import java.io.File;

fn read_file(path: String) -> Result<String, JavaException> {
    safe {
        let file = File(path);
        // Java メソッド呼び出し — 例外は自動で Result に変換
        file.readString()
    }
}
```

**interop 固有の規則:**

- `safe {}` 内で呼び出せるのは Java メソッド・コンストラクタに限定されない（Valen コードも記述可能）が、例外変換の対象は Java 由来の例外のみ
- Valen コード内の panic は `safe {}` で捕捉されない（panic は契約違反であり回復対象外）
- `safe {}` ブロックは Java interop のあらゆる場所で使用を推奨するが、強制ではない（`safe {}` 外で Java メソッドを呼ぶとどうなるかは REQ-FAIL-004 の設計判断に委ねる）

### 受入条件

- [ ] `safe {}` 内で Java メソッドが呼び出せる
- [ ] Java checked exception が `Err(JavaException)` に変換される
- [ ] Java unchecked exception が `Err(JavaException)` に変換される
- [ ] 例外が発生しない場合に `Ok(value)` が返される
- [ ] `JavaException` から元の例外のクラス名・メッセージ・スタックトレースにアクセス可能
- [ ] `safe {}` 内の Valen panic が捕捉されずプロセスが停止する
- [ ] ネストした Java メソッド呼び出しの例外が正しく捕捉される
- [ ] `safe {}` の戻り値型が `Result<T, JavaException>` として型推論される

### 依存

- REQ-FAIL-004（safe {} ブロックの言語仕様）
- REQ-FAIL-005（safe {} 内 Java 戻り値の T? 変換）
- REQ-INTEROP-001（Java クラスの import と利用）

---

## REQ-INTEROP-003: `@valen.Closed` で Java sealed 型を exhaustive match

| 項目 | 内容 |
|------|------|
| **ID** | REQ-INTEROP-003 |
| **優先度** | Must |
| **ステータス** | Draft |
| **Phase** | MVP |

### 説明

Java の sealed class/interface を Valen の exhaustive match で利用するための仕組み。Java 側に `@valen.Closed` アノテーションを付与することで、Valen コンパイラがその型の全サブタイプを把握し、match 式で exhaustive check を実施する。

**Java 側:**
```java
@valen.Closed
public sealed interface Shape permits Circle, Rectangle, Point {}
public record Circle(double radius) implements Shape {}
public record Rectangle(double width, double height) implements Shape {}
public record Point() implements Shape {}
```

**Valen 側:**
```valen
import com.example.Shape;

fn describe(shape: Shape) -> String {
    match shape {
        Circle(r) => f"Circle with radius {r}",
        Rectangle(w, h) => f"Rectangle {w}x{h}",
        Point => "Point",
    }
    // ← ワイルドカード不要（exhaustive）
}
```

**制約:**
- `@valen.Closed` が付与されていない Java sealed 型は exhaustive 対象外（ワイルドカード `_` が必須）
- `@valen.Closed` は Valen が提供する Java annotation（`valen-annotations.jar`）
- コンパイル時に permits リストを解析し、全サブタイプの網羅を検証する
- Java 側でサブタイプが追加された場合、Valen 側の match がコンパイルエラーになる（意図通り）

### 受入条件

- [ ] `@valen.Closed` 付き Java sealed 型に対して exhaustive match が動作する
- [ ] 全サブタイプを網羅した match でワイルドカード `_` が不要
- [ ] サブタイプの漏れがある match でコンパイルエラーが報告される
- [ ] `@valen.Closed` なし Java sealed 型に対して `_` 必須が強制される
- [ ] `@valen.Closed` なし Java sealed 型で `_` を省略するとコンパイルエラー
- [ ] Java record のコンポーネントに対する構造分解が動作する
- [ ] `valen-annotations.jar` が提供され、Java プロジェクトの依存に追加可能
- [ ] Java 側でサブタイプが追加された後に Valen を再コンパイルするとエラーになる

### 依存

- REQ-ADT-002（exhaustive match の仕様）
- REQ-INTEROP-001（Java クラスの import と利用）
- REQ-EMIT-001（Java 21 class file — sealed class サポート前提）

---

## 変更履歴

| 日付 | 変更内容 | 担当 |
|------|---------|------|
| 2026-05-11 | 初版作成 | — |
