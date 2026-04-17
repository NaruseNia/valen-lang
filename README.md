# Valen

**An ADT-first JVM language — not OO with ADTs bolted on, but ADTs dropped onto the JVM.**

English | [日本語](README_ja.md)

Valen is an ADT-centric JVM language that rides on Java and Kotlin's existing ecosystem. Its four pillars are strong algebraic data types, exhaustive `match`, trait-based abstraction, and a coherent `Option` / `Result` failure model — expressed without breaking the Java / Kotlin world around it.

Valen does not try to beat Kotlin. It aims to be the complementary choice for people who want real ADTs on the JVM, in the smallest form that still delivers them.

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
    }
}
```

## Features

- **Algebraic data types and exhaustive match** — `enum` is a Rust-style ADT; `match` is the full Rust set (destructuring, guards, ranges, or-patterns, `@` bindings, exhaustiveness check).
- **Coherent failure model** — `Option` for absence, `Result` for recoverable failure, `Exception` for FFI boundary errors, `panic` for contract violation. Each role is distinct, and `?` performs early return.
- **Trait-based abstraction** — strict orphan rule, globally unique `(trait, type)` pairs, blanket impls disallowed in the MVP.
- **Seamless Java / Kotlin interop** — `import java.util.List;`, explicit conversion for Java exceptions, compatibility with Java sealed hierarchies.
- **Modern syntax** — `fn`, `let` / `let mut`, `match`, `::` for enum variants and associated functions, `.` for member access.
- **JVM 21 baseline, 25 opt-in** — new features such as Valhalla value types are gated behind `--target 25`.

## Status

In design. See [docs/LANGUAGE_SPEC.md](docs/LANGUAGE_SPEC.md) and [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md).

## Build (future)

```sh
# valenc (compiler, Rust implementation)
cargo build --release --bin valenc

# Gradle plugin (Kotlin implementation)
./gradlew :valen-gradle-plugin:build

# Minimal LSP
cargo build --release --bin valen-lsp
```

## License

[Apache License 2.0](LICENSE)
