# 8. 失敗モデル

## 8.1 役割分離

| 機構 | 用途 |
|------|------|
| `Option<T>` | 値の欠如専用 |
| `Result<T, E>` | 回復可能失敗（`E: Error` 制約あり） |
| `panic` | 契約違反・到達不能・処理継続不能の停止機構 |
| Exception | FFI 境界の異常のみ |

**Valen 内で `throw` 文は禁止**。ドメイン失敗は Option / Result で表現、異常停止は `panic!` を使う。

## 8.2 Error trait

`valen.core` に標準 `Error` trait を定義する。`Result<T, E>` の `E` は `Error` を実装しなければならない。

```valen
trait Error {
    fn message(self) -> String;
}
```

ユーザ定義エラー型は `Error` を実装する：

```valen
enum AppError {
    NotFound(id: Int),
    Forbidden(reason: String),
}

impl Error for AppError {
    fn message(self) -> String {
        match self {
            AppError::NotFound(id) => f"not found: {id}",
            AppError::Forbidden(reason) => f"forbidden: {reason}",
        }
    }
}
```

## 8.3 `?` 演算子

- `Result<T, E>` 上で使用（`Ok(v) → v`、`Err(e) → early return Err(e)`）
- **`?` は E 型が同一の場合のみ伝播する。** 異なるエラー型間の自動変換は行わない。`map_err` で明示変換する
- `Option<T>` 上は **戻り値が `Option<U>` の関数内のみ**で使える
- `Option → Result` 暗黙昇格は禁止

```valen
fn find_user(id: Int) -> Result<User, DbError> {
    let row = query(id)?;  // DbError → DbError（同一型、OK）
    Ok(User::from_row(row))
}

// 異なるエラー型は map_err で変換
fn load(path: String) -> Result<Data, AppError> {
    let content = read_file(path)
        .map_err(|e| AppError::IoFailed(e.message()))?;
    parse(content)
        .map_err(|e| AppError::ParseFailed(e.message()))
}

fn first_char(s: String) -> Option<Char> {
    let c = s.chars().first()?;  // Option 上（関数の戻りも Option）
    Some(c.to_uppercase())
}
```

## 8.4 Java exception 境界

**自動ラップなし、明示変換**。

MVP では `safe { ... }` ブロック方式を必須とする：

```valen
fn read_safe(path: String) -> Result<String, JavaException> {
    safe { java.nio.file.Files.readString(java.nio.file.Paths.get(path)) }
}
```

### Java null の扱い

`safe { }` ブロック内の Java メソッド戻り値は**自動的に `T?`（`Option<T>`）として型付け**される。`void` メソッドは `Unit` のまま。

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

**根拠:** Java メソッドの戻り値は常に null 可能性がある（`@NonNull` annotation があっても保証されない）。Valen の芯「整合した失敗モデル」に従い、曖昧さを排除する。Kotlin の platform type (`T!`) のような判断遅延は採用しない。

### Phase 1.5+ 検討

- 方針 A: `unsafe fn` で生呼び出し
- 方針 C: `@catch` attribute で opt-in
