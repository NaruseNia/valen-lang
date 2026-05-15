# 11. 並行

## 11.1 MVP

MVP では **virtual thread (Loom) 一本**。

```valen
fn main() {
    let t = virtualThread { expensive_op() };
    t.join();
}
```

## 11.2 Phase 2+

`suspend fn` / async モデルは Phase 2 で再評価。MVP にはない。
