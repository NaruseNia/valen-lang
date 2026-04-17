# Valen

**OOの上にADTを足すのでなく、ADTを中核に据えてJVMへ落とす言語。**

Valen は Java/Kotlin 資産に乗る、ADT 中心の JVM 言語です。強い代数的データ型、exhaustive な `match`、trait ベースの抽象、整合した `Option`/`Result` 失敗モデル — この4点を芯として、Java と Kotlin の既存エコシステムを壊さずに表現します。

Valen は Kotlin 超えを主張しません。補完的な選択肢として、「ADT が本当に強い JVM 言語」を最小限の形で提供することを目標にしています。

---

## Hello, Valen

```valen
package com.example.hello;

import java.util.List;

data class User(name: String, mut age: Int);

enum Shape {
    Circle(r: Float),
    Rect(w: Float, h: Float),
    Point,
}

trait Area {
    fn area(self) -> Float;
}

impl Area for Shape {
    fn area(self) -> Float {
        match self {
            Shape::Circle(r) => 3.14159 * r * r,
            Shape::Rect(w, h) => w * h,
            Shape::Point => 0.0,
        }
    }
}

fn main() {
    let shapes: List<Shape> = List.of(
        Shape::Circle(r = 5.0),
        Shape::Rect(w = 3.0, h = 4.0),
        Shape::Point,
    );

    for s in shapes {
        println(f"area = {s.area()}")
    };
}
```

## 特徴

- **代数的データ型と exhaustive match** — `enum` は Rust 型の ADT、`match` は Rust フルセット（構造分解・ガード・範囲・or パターン・`@` 束縛・exhaustive check）
- **整合した失敗モデル** — `Option = 欠如`、`Result = 回復可能失敗`、`Exception = FFI 境界の異常`、`panic = 契約違反`。役割が明確に分離、`?` 演算子で early return
- **trait ベース抽象** — orphan rule 厳格、同一 trait/type 対はグローバル一意、blanket impl 禁止（MVP）
- **Java/Kotlin 完全相互運用** — `import java.util.List;`、Java exception 明示変換、Java sealed hierarchy と互換
- **モダン構文** — `fn`, `let` / `let mut`, `match`, `::` (enum variant) + `.` (member)
- **JVM 21 baseline / 25 opt-in** — Valhalla 等の新機能は `--target 25` でオプトイン

## ステータス

設計段階。[docs/LANGUAGE_SPEC.md](docs/LANGUAGE_SPEC.md) と [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) を参照。

## ビルド（将来）

```sh
# valenc（コンパイラ、Rust 実装）
cargo build --release --bin valenc

# Gradle プラグイン（Kotlin 実装）
./gradlew :valen-gradle-plugin:build

# 最小型 LSP
cargo build --release --bin valen-lsp
```

## ライセンス

[Apache License 2.0](LICENSE)
