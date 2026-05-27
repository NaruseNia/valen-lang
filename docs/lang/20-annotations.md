# 20. アノテーション

## 20.1 構文

アノテーションは `@Foo` 形式（Java 流）。JVM エコシステムと相互運用最優先。

```valen
@Foo
@Bar(key = "value")
```

`::` / `.` と並ぶ意味のある prefix 記号として `@` を予約する。

## 20.2 annotation class 宣言

```valen
annotation class Deprecated(pub message: String)

annotation class Serializable  // マーカー annotation（パラメータなし）

@Target("type", "field")
annotation class JsonName(pub name: String)
```

**構文:** `annotation class Name(params)` — `annotation` は予約キーワード。パーサーは `annotation class` の連続トークンを `AnnotationClassDecl` として認識する。

**パラメータ:** 各パラメータは `visibility name: Type` 形式。可視性修飾子付き。

**パラメータの型:** 型パーサーが受理する任意の型を書けるが、意味的にはリテラル型（String, Int, Float, Bool, Long, Double, Char）が前提。

**@Target 指定:** `@Target("type")` / `@Target("type", "field", "method")` で制限可能。パーサーが `@Target` アノテーションの引数を読み取り、resolver が `AnnotationClassDef.targets` フィールドに格納する。

> **実装状況（@Target 検証）:** `targets` は HIR に保存されるが、**アノテーション適用先の妥当性検証は未実装**。例えばフィールド専用アノテーションをクラスに付けてもコンパイルエラーにならない。

**@Retention:** 仕様上のデフォルトは RUNTIME retention。

> **実装状況（@Retention）:** retention の解析・保存・JVM バイトコード反映はすべて未実装。

**JVM ABI:** `@interface`（ACC_INTERFACE | ACC_ABSTRACT | ACC_ANNOTATION）として emit する設計。

## 20.2.1 annotation 適用

```valen
@Deprecated(message = "use NewApi")
pub class OldApi {}

@JsonName("user_name")     // 単一パラメータは名前省略可
pub name: String

@Serializable              // マーカーは () 不要
data class User(pub name: String);
```

**適用対象:** トップレベル宣言（class, data class, enum, trait, fn）+ コンストラクタパラメータ（`CtorParam`）+ フィールド（`FieldDecl`）。

パーサーは `@Name(args)` 形式をパースし、各宣言の `annotations: Vec<Annotation>` フィールドに格納する。

**引数構文:** named 引数基本（`key = value`）。パラメータが1つのみの場合は名前省略可（`@Foo("bar")`）。引数の値はリテラルのみ（`parse_literal()` で解析: Int, Long, Float, Double, String, Char, Bool）。

**Java annotation の適用について:**

> **実装状況:** パーサーは Valen で定義された annotation class の宣言と、Valen コード上での `@Foo(...)` 適用構文をパースする。import した Java annotation を `@Foo(...)` で適用する構文は同一だが、**パラメータの型検証は行わない**（信頼ベース）。Java annotation 定義自体の読み込み・解決は classpath 連携が必要で、現時点では行われていない。

## 20.3 `@valen.Closed`

Java sealed hierarchy を Valen から exhaustive match 可能にする唯一の builtin annotation。

**位置づけ:**

- Valen が Java annotation として公開（`package valen; @interface Closed`）
- Java ライブラリ作者が sealed interface / sealed class に付与する
- Valen 側のコードに `@closed` / `@valen.Closed` を書くことはできない

**ターゲット:**

- Java `sealed interface`
- Java `sealed class`

それ以外（enum / interface / class）への付与は未定義。

**効果:** Valen コンパイラは `@valen.Closed` 付きの Java sealed hierarchy を closed-world として扱い、`match` で exhaustive check を有効化する。`has_valen_closed` フラグと `permitted_subclasses` リストを HIR の `foreign_types` に格納し、exhaustiveness checker がこれを参照する。

```java
// Java 側定義（ライブラリ作者）
package com.example;

import valen.Closed;

@Closed
public sealed interface Color permits Red, Blue, Green {}
```

```valen
// Valen 側使用
import com.example.Color;

match color {
    Color.Red => ...,
    Color.Blue => ...,
    Color.Green => ...,  // 網羅しないとコンパイルエラー
}
```

## 20.4 `@valen.Closed` 不在時の挙動

**Java `sealed` 単独では exhaustive 扱いにしない**。`@valen.Closed` の付与がない Java hierarchy は常に open-world と判定し、`match` では wildcard arm (`_`) を要求する。

```valen
// @valen.Closed が付いていない Java sealed interface
match color {
    Color.Red => ...,
    Color.Blue => ...,
    Color.Green => ...,
    _ => ...,  // 必須、省くとコンパイルエラー
}
```

**設計意図:** Valen が自分で定義した closed world（`enum` / `sealed class`）は compiler が完全に把握しているので exhaustive check を厳密にできる。Java 定義の closed world は classpath 変動・tooling 差異があるため、ライブラリ作者の明示 opt-in を要求する。
