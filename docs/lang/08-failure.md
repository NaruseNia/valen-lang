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

## 8.5 `unsafe` ブロック / `unsafe fn`

`unsafe` は Valen の型・失敗モデルの安全保証を明示的に bypass する機構。通常の `safe` パスでは保証される例外ラップ・null 正規化・キャスト検証を全てスキップする。

### unsafe ブロック式

`unsafe { expr }` はブロック式であり、最後の式の値を返す。

```valen
let pos: Position = unsafe { obj as Position };
```

### unsafe 短縮構文

1行の式には `unsafe expr` の短縮形が使える。`unsafe { expr }` と等価。

```valen
let pos: Position = unsafe obj as Position;
```

### `unsafe fn`

関数宣言に `unsafe` を付けると、本体全体が暗黙の unsafe コンテキストになる。`unsafe fn` の呼び出しは `unsafe { }` ブロック内でのみ許可される。

```valen
unsafe fn rawAccess(ptr: Long) -> Int { ... }

// 呼び出し側
let v = unsafe { rawAccess(ptr) };
```

### unsafe 内で許可される操作

1. **unchecked downcast** — `obj as ConcreteType`（ClassCastException リスク）
2. **Java exception 無視** — Java メソッド呼び出しの例外を catch せず素通り
3. **non-nullable null** — `let x: String = unsafe { null };`

## 8.6 `safe` 短縮構文 / `safe?` 合体構文

### `safe expr`

`safe { expr }` の短縮形。結果は `Result<T?, JavaException>`。

```valen
let r = safe file.readString();  // Result<String?, JavaException>
```

### `safe? expr`

`safe { expr }?` と等価。`Result` を `?` で早期 return し、`T?` を返す。

```valen
let s: String? = safe? file.readString();
// ↑ safe { file.readString() }? と同じ
```

## 8.7 `as` キャスト式

`expr as Type` で型キャストを行う。安全性は変換の種類で決まる。

- **safe（unsafe 不要）:** 数値 widening（`42 as Long`、`Int` → `Long` 等）
- **unsafe 必須:** ダウンキャスト（`obj as Position` — ClassCastException リスク）

```valen
// 安全な widening — unsafe 不要
let x: Long = 42 as Long;

// ダウンキャスト — unsafe 必須
let pos: Position = unsafe { obj as Position };
```

## 8.8 Java メソッド呼び出しモード

Java メソッド呼び出しは `safe` か `unsafe` で囲む必要がある。素呼び出しはコンパイルエラー。

| 記法 | 戻り値型 | 例外処理 | null 処理 |
|------|---------|---------|----------|
| `safe { expr }` / `safe expr` | `Result<T?, JavaException>` | `Err` にラップ | `T?`（nullable） |
| `safe? expr` | `T?` | 早期 return | `T?`（nullable） |
| `unsafe { expr }` / `unsafe expr` | `T`（non-nullable） | 素通り（crash） | NPE リスク |
| 素呼び出し | **コンパイルエラー** | — | — |

**例外:** Java コンストラクタ呼び出しは `safe`/`unsafe` 不要。コンストラクタは必ず non-null を返し、例外発生時はそもそもオブジェクトが生成されないため。

```valen
let list = ArrayList();  // safe/unsafe 不要
```
