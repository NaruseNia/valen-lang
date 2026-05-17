# 失敗モデル

Valen は4つの失敗機構を明確に役割分離しています。それぞれの用途を理解し、正しく使い分けることが Valen らしいコードを書く鍵です。

## 4つの失敗機構

| 機構 | 用途 | いつ使うか |
|------|------|-----------|
| `Option<T>` | 値の欠如 | 値がないことが正常な場合（検索結果が0件など） |
| `Result<T, E>` | 回復可能な失敗 | 呼び出し側が失敗に対処できる場合（ファイル読み込みなど） |
| `panic` | 契約違反 | プログラムのバグ、到達不能、処理を続行できない場合 |
| Exception | FFI 境界の異常 | Java メソッド呼び出しで発生する例外のみ |

**Valen 内で `throw` は禁止です。** ドメインの失敗は `Option` / `Result` で表現し、プログラムのバグは `panic` で停止します。

## Option — 値の欠如

`Option<T>` は「値があるかもしれないし、ないかもしれない」を表す型です。`T?` は `Option<T>` の糖衣構文です。

```valen
fn find_user(id: Int) -> Option<User> {
    if id == 1 {
        Some(User(name = "Alice", age = 30))
    } else {
        None
    }
}

// 使う側
let user = find_user(42);
match user {
    Some(u) => println(f"Found: {u.name}"),
    None => println("User not found"),
}
```

### Option のメソッド

`Option<T>` には以下の inherent メソッドがあります。

| メソッド | シグネチャ | 説明 |
|----------|-----------|------|
| `map` | `fn map<U>(self, f: fn(T) -> U) -> Option<U>` | Some の中身を変換 |
| `flatMap` | `fn flatMap<U>(self, f: fn(T) -> Option<U>) -> Option<U>` | Some の中身を Option に変換して flatten |
| `unwrapOr` | `fn unwrapOr(self, default: T) -> T` | Some なら中身、None ならデフォルト値 |
| `filter` | `fn filter(self, predicate: fn(T) -> Bool) -> Option<T>` | 条件を満たさなければ None |
| `isSome` | `fn isSome(self) -> Bool` | Some なら true |
| `isNone` | `fn isNone(self) -> Bool` | None なら true |

### T? 構文

型注釈で `T?` と書くと `Option<T>` と同じ意味になります。短く書きたい場合に便利です。

```valen
fn first_name(full_name: String?) -> String {
    match full_name {
        Some(name) => name,
        None => "Anonymous",
    }
}
```

## Result — 回復可能な失敗

`Result<T, E>` は「成功（`Ok(T)`）か失敗（`Err(E)`）のどちらか」を表す型です。`E` は `Error` trait を実装している必要があります。

### Result のメソッド

`Result<T, E>` には以下の inherent メソッドがあります。

| メソッド | シグネチャ | 説明 |
|----------|-----------|------|
| `map` | `fn map<U>(self, f: fn(T) -> U) -> Result<U, E>` | Ok の中身を変換 |
| `mapErr` | `fn mapErr<F>(self, f: fn(E) -> F) -> Result<T, F>` | Err の中身を変換 |
| `flatMap` | `fn flatMap<U>(self, f: fn(T) -> Result<U, E>) -> Result<U, E>` | Ok をチェーン |
| `unwrapOr` | `fn unwrapOr(self, default: T) -> T` | Ok なら中身、Err ならデフォルト値 |
| `isOk` | `fn isOk(self) -> Bool` | Ok なら true |
| `isErr` | `fn isErr(self) -> Bool` | Err なら true |

### Error trait の定義

```valen
trait Error {
    fn message(self) -> String;
}
```

### ユーザ定義エラー

enum で独自のエラー型を定義し、`Error` trait を実装します。

```valen
enum AppError {
    NotFound(id: Int),
    Forbidden(reason: String),
    IoFailed(detail: String),
}

impl Error for AppError {
    fn message(self) -> String {
        match self {
            AppError::NotFound(id) => f"not found: {id}",
            AppError::Forbidden(reason) => f"forbidden: {reason}",
            AppError::IoFailed(detail) => f"I/O error: {detail}",
        }
    }
}
```

### Result の使い方

```valen
fn load_config(path: String) -> Result<Config, AppError> {
    let content = read_file(path);
    match content {
        Ok(text) => parse_config(text),
        Err(e) => Err(AppError::IoFailed(detail = e.message())),
    }
}
```

## ? 演算子

`?` 演算子は `Result` や `Option` からの早期リターンを簡潔に書くためのものです。

### Result 上の ?

`Result<T, E>` に `?` を使うと、`Ok(v)` なら値 `v` を取り出し、`Err(e)` なら即座に `Err(e)` を返します。

```valen
fn find_user(id: Int) -> Result<User, DbError> {
    let row = query(id)?;  // Err なら即 return Err(e)
    Ok(User::from_row(row))
}
```

**重要: `?` は同一のエラー型でのみ伝播します。** 異なるエラー型間の自動変換は行われません。エラー型が異なる場合は `map_err` で明示的に変換してください。

```valen
fn load(path: String) -> Result<Data, AppError> {
    // read_file は Result<String, IoError> を返す
    // AppError に変換してから ? で伝播
    let content = read_file(path)
        .map_err(|e| AppError::IoFailed(detail = e.message()))?;

    // parse は Result<Data, ParseError> を返す
    parse(content)
        .map_err(|e| AppError::IoFailed(detail = e.message()))
}
```

### Option 上の ?

`Option<T>` に `?` を使うこともできます。ただし、関数の戻り値が `Option<U>` の場合に限ります。`Some(v)` なら値 `v` を取り出し、`None` なら即座に `None` を返します。

```valen
fn first_char_upper(s: String) -> Option<Char> {
    let c = s.chars().first()?;  // None なら即 return None
    Some(c.to_uppercase())
}
```

### Option → Result の暗黙昇格は禁止

`Option` の値を `Result` を返す関数内で `?` することはできません。明示的に変換してください。

## safe { } ブロック — Java 例外の捕捉

Java メソッドは例外を投げる可能性があります。Valen では `safe { }` ブロックを使って Java メソッドを呼び出し、例外を `Result<T, JavaException>` に自動変換します。

```valen
fn read_safe(path: String) -> Result<String, JavaException> {
    safe { java.nio.file.Files.readString(java.nio.file.Paths.get(path)) }
}
```

`safe { }` ブロックの中で Java メソッドが例外を投げると、その例外は `Err(JavaException)` として返されます。例外が起きなければ `Ok(value)` が返ります。

### safe 内の Java 戻り値は自動的に T?

Java メソッドの戻り値は null を返す可能性があるため、`safe { }` ブロック内の Java メソッド戻り値は自動的に `T?`（`Option<T>`）として型付けされます。`void` メソッドは `Unit` のままです。

```valen
// Java: V Map.get(K key) — null を返す可能性あり
let val: Option<String> = safe { map.get("key") };
match val {
    Some(v) => println(v),
    None => println("not found"),
}

// void メソッドは Unit
safe { list.add("item") };  // Unit
```

Kotlin の platform type (`T!`) のような「null かもしれないし、null じゃないかもしれない」という曖昧な型は採用しません。Java の戻り値は常に null の可能性があるものとして扱います。

### safe と ? の組み合わせ

`safe { }` の結果は `Result` なので、`?` 演算子と組み合わせて使えます。

```valen
fn process_file(path: String) -> Result<Int, JavaException> {
    let content = safe { java.nio.file.Files.readString(
        java.nio.file.Paths.get(path)
    ) }?;

    match content {
        Some(text) => Ok(text.length()),
        None => Ok(0),
    }
}
```

## まとめ: どれを使うか

```
値がないかも？             → Option<T>
失敗するかも、呼び出し側で対処？ → Result<T, E>
バグ、到達不能？            → panic
Java メソッドの例外？       → safe { } で Result に変換
```

Valen では「失敗がどの種類か」を型で表現します。`throw` による暗黙の制御フロー変更はなく、失敗の可能性は常にシグネチャから読み取れます。
