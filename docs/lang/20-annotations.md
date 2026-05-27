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

**構文:** `annotation class Name(params)` — `annotation` は予約キーワード。

**パラメータ値:** リテラルのみ（String, Int, Float, Bool, Long, Double, Char）。

**retention:** デフォルト RUNTIME。`@Retention(RUNTIME)` + `@Target(...)` が自動 emit される。

**@Target 指定:** `@Target("type")` / `@Target("type", "field", "method")` で制限可能。未指定時は TYPE + FIELD + METHOD。

**JVM ABI:** `@interface`（ACC_INTERFACE | ACC_ABSTRACT | ACC_ANNOTATION）として emit。

## 20.2.1 annotation 適用

```valen
@Deprecated(message = "use NewApi")
pub class OldApi {}

@JsonName("user_name")     // 単一パラメータは名前省略可
pub name: String

@Serializable              // マーカーは () 不要
data class User(pub name: String);
```

**適用対象:** トップレベル宣言（class, data class, enum, trait, fn）+ フィールド / ctor パラメータ。

**引数構文:** named 引数基本（`key = value`）。パラメータが1つのみの場合は名前省略可（`@Foo("bar")`）。

**Java annotation:** import した Java annotation も `@Foo(...)` で適用可能。パラメータ検証なし（信頼ベース emit）。

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

**効果:** Valen コンパイラは `@valen.Closed` 付きの Java sealed hierarchy を closed-world として扱い、`match` で exhaustive check を有効化する。

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

## 20.5 Java annotation の扱い

Valen コード側から Java annotation を直接付与することは現在サポートしていない。Java ライブラリ側で annotation を付ける必要がある場合は、Java ソースを直接編集する。
