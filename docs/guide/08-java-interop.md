# Java 相互運用

Valen は JVM 上で動作し、Java のライブラリやフレームワークをそのまま利用できます。この章では Java コードとの相互運用の仕組みを説明します。

## import

Java のクラスを Valen から使うには `import` 文でインポートします。

```valen
import java.util.List;
import java.util.HashMap;
import java.util.concurrent.ConcurrentHashMap as CMap;
```

- `import path.to.Type;` — 型を単一インポート
- `import path.to.Type as Alias;` — 別名を付けてインポート

別名（alias）は長いクラス名を短くしたい場合や、名前の衝突を避けたい場合に便利です。

```valen
import java.util.HashMap as HMap;

fn example() -> Result<Unit, JavaException> {
    let map = safe { HMap::new() };
    // ...
    Ok(())
}
```

## safe { } ブロックでの Java メソッド呼び出し

Java メソッドは例外を投げる可能性があるため、Valen では `safe { }` ブロック内で呼び出します。ブロック内で発生した Java 例外は自動的に `Result<T, JavaException>` に変換されます。

```valen
fn read_file(path: String) -> Result<String?, JavaException> {
    safe { java.nio.file.Files.readString(java.nio.file.Paths.get(path)) }
}
```

- 例外が発生しなければ `Ok(value)` が返ります
- 例外が発生すると `Err(JavaException)` が返ります

`safe { }` の結果は `Result` なので、`?` 演算子でエラーを伝播できます。

```valen
fn count_lines(path: String) -> Result<Int, JavaException> {
    let content = safe {
        java.nio.file.Files.readString(java.nio.file.Paths.get(path))
    }?;

    match content {
        Some(text) => Ok(text.lines().count()),
        None => Ok(0),
    }
}
```

## Java null の扱い

Java メソッドの戻り値は null を返す可能性があります。Valen では `safe { }` ブロック内の Java メソッド戻り値を**自動的に `T?`（`Option<T>`）**として型付けします。

```valen
import java.util.HashMap;

fn lookup() -> Result<Unit, JavaException> {
    let map = safe { HashMap::new() }?;
    safe { map.put("key", "value") };

    // map.get() は Java では V を返すが、Valen では Option<String> になる
    let val: Option<String> = safe { map.get("key") }?;

    match val {
        Some(v) => println(f"found: {v}"),
        None => println("not found"),
    }

    Ok(())
}
```

`void` を返す Java メソッドは `Unit` のままです。

```valen
// void メソッド → Unit
safe { list.add("item") };
```

Kotlin のように null かどうかを曖昧にする platform type (`T!`) は採用しません。Java 由来の値は常に「null の可能性がある」として安全に扱います。

## @valen.Closed — Java sealed hierarchy の網羅性検査

Valen の `enum`、`sealed class`、`sealed trait` は `match` で厳密な網羅性検査（exhaustive check）が行われます。では Java の `sealed interface` はどうでしょうか？

### デフォルト: open-world 扱い

Java の `sealed` 型は、デフォルトでは open-world（開いた世界）として扱われます。`match` では必ず `_`（ワイルドカード）が必要です。

```valen
// Java側: sealed interface Color permits Red, Blue, Green
// @valen.Closed なし

match color {
    Color.Red => "赤",
    Color.Blue => "青",
    Color.Green => "緑",
    _ => "不明",  // 省くとコンパイルエラー
}
```

理由: Valen コンパイラは Java の classpath 上にある型を完全には把握できません。classpath が変わって permit 先が増えたとき、気づかないうちに `match` が非網羅になる事故を防ぎます。

### @valen.Closed で exhaustive match を有効化

Java ライブラリの作者が sealed hierarchy に `@valen.Closed` アノテーションを付与すると、Valen からの exhaustive match が有効になります。

```java
// Java 側定義（ライブラリ作者が付与）
package com.example;

import valen.Closed;

@Closed
public sealed interface Color permits Red, Blue, Green {}
public final class Red implements Color {}
public final class Blue implements Color {}
public final class Green implements Color {}
```

```valen
// Valen 側 — @valen.Closed が付いているので exhaustive check が有効
import com.example.Color;

match color {
    Color.Red => "赤",
    Color.Blue => "青",
    Color.Green => "緑",
    // 全 permit を網羅しているので OK（_ 不要）
}
```

**ポイント:**

- `@valen.Closed` は Java 側に付けるアノテーションです。Valen のコードには書きません
- ライブラリ作者が「この sealed hierarchy は安定しており、permit 先が増えることはない」と保証する意思表示です
- `@valen.Closed` がなければ、Java の `sealed` であっても `_` が必須です

## classpath の設定

`valenc` は Java の `.class` ファイルを classpath から読み取り、Java 型の情報を取得します。

```sh
valenc compile --classpath lib/guava.jar:lib/commons.jar src/main.vln
```

- `--classpath`（または `-cp`）で JAR ファイルやディレクトリを指定します
- 複数のパスは `:`（Linux/macOS）または `;`（Windows）で区切ります
- JDK の標準ライブラリ（`java.lang`、`java.util` 等）は自動的に参照できます

## valen.collections — 標準コレクション

Valen は標準的なコレクション型として `List`、`Map`、`Set` を提供しますが、これらは `java.util` パッケージの型への typealias です。

```valen
// List, Map, Set は import なしで使える（prelude に含まれる）
let names: List<String> = List::of("Alice", "Bob", "Charlie");
let scores: Map<String, Int> = Map::of("Alice", 100, "Bob", 85);
let tags: Set<String> = Set::of("valen", "jvm", "language");
```

Java のコレクションライブラリがそのまま使えるため、既存の Java エコシステムとの親和性が高くなっています。

## まとめ

| 操作 | 方法 |
|------|------|
| Java クラスの利用 | `import java.util.List;` |
| 名前の衝突回避 | `import ... as Alias;` |
| Java メソッド呼び出し | `safe { javaMethod() }` → `Result<T, JavaException>` |
| null の扱い | `safe` 内の戻り値は自動で `T?` |
| sealed の網羅性 | `@valen.Closed` 付きなら exhaustive、なしなら `_` 必須 |
| classpath 指定 | `valenc compile --classpath ...` |
