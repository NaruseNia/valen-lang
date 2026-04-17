# 8. 失敗モデル

## 8.1 役割分離

| 機構 | 用途 |
|------|------|
| `Option<T>` | 値の欠如専用 |
| `Result<T, E>` | 回復可能失敗 |
| `panic` | 契約違反・到達不能・処理継続不能の停止機構 |
| Exception | FFI 境界の異常のみ |

**Valen 内で `throw` 文は禁止**。ドメイン失敗は Option / Result で表現、異常停止は `panic!` を使う。

## 8.2 `?` 演算子

- `Result<T, E>` 上で維持（`Ok(v) → v`、`Err(e) → early return Err(e)`）
- `Option<T>` 上は **戻り値が `Option<U>` の関数内のみ**で使える
- `Option → Result` 暗黙昇格は禁止

```valen
fn find_user(id: Int) -> Result<User, AppError> {
    let row = query(id)?;  // Result 上
    Ok(User::from_row(row))
}

fn first_char(s: String) -> Option<Char> {
    let c = s.chars().first()?;  // Option 上（関数の戻りも Option）
    Some(c.to_uppercase())
}
```

## 8.3 Java exception 境界

**自動ラップなし、明示変換**。

```valen
// 方針 A: unsafe fn で生呼び出し
unsafe fn read_raw(path: String) -> String {
    java.nio.file.Files.readString(java.nio.file.Paths.get(path))  // 例外は素通し
}

// 方針 B: safe ラッパで Result 包装
fn read_safe(path: String) -> Result<String, JavaException> {
    safe { java.nio.file.Files.readString(java.nio.file.Paths.get(path)) }
}

// 方針 C: @catch attribute で opt-in
@catch(IOException)
fn read(path: String) -> Result<String, IOException> {
    java.nio.file.Files.readString(java.nio.file.Paths.get(path))
}
```

MVP では B 方針（`safe { ... }` ブロック）を必須、A / C は Phase 1.5+ で検討。
