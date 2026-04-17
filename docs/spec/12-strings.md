# 12. 文字列

## 12.1 リテラル

```valen
let s = "hello";
let raw = r"raw\nstring";  // raw literal（\n は文字通り）
```

## 12.2 補間（MVP）

```valen
let name = "Alice";
let age = 30;
let msg = f"Hello, {name}! You are {age} years old.";
let expr = f"2 + 2 = {2 + 2}";
```

## 12.3 複数行（Phase 1.5）

```valen
// Phase 1.5 以降
let doc = f"""
    name: {name}
    age: {age}
""";
```
