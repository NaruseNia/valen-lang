# 13. コレクション / for

## 13.1 MVP の扱い

`List` / `Map` / `Set` は **`java.util` の typealias**：

```valen
typealias List<T> = java.util.List<T>;
typealias Map<K, V> = java.util.Map<K, V>;
typealias Set<T> = java.util.Set<T>;
```

trait 注入は **iteration, map, filter 程度に最小化**（MVP）。

## 13.2 for-in

```valen
for x in xs {
    println(x);
};
```

- `Iterator` trait を実装した型に適用
- `java.lang.Iterable` は自動アダプト

## 13.3 Iterator trait

```valen
trait Iterator<T> {
    fn next(mut self) -> Option<T>;
}
```
