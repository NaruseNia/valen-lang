# 20. アノテーション

## 20.1 構文

アノテーションは `@Foo` 形式（Java 流）。JVM エコシステムと相互運用最優先。

```valen
@Foo
@Bar(key = "value")
```

`::` / `.` と並ぶ意味のある prefix 記号として `@` を予約する。

## 20.2 MVP スコープ

**MVP では Valen コード内に annotation を書けない**。`@` トークンは lexer で予約されるが、parser は annotation 付与位置で受け付けない。

唯一の例外は Valen が提供する Java 側 annotation の **読み取り** — `@valen.Closed` のみ。

**Phase 1.5+ で追加予定:**

- annotation 宣言構文（`annotation class` など）
- annotation の読み取り API（reflection 以外）
- Valen コード内での annotation 付与

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

MVP では、Valen コード側から Java annotation を付与することはできない。Java ライブラリ側で annotation を付ける必要がある場合は、Java ソースを直接編集する。

Phase 1.5+ で Valen コードからの Java annotation 付与をサポートする予定。
