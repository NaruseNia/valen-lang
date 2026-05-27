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

- **Algebraic data types and exhaustive match** — `enum` is a Rust-style ADT; `match` supports destructuring, guards, ranges, or-patterns, `@` bindings, and exhaustiveness checking.
- **Coherent failure model** — `Option` for absence, `Result` for recoverable failure, `Exception` for FFI boundary, `panic` for contract violation. `?` performs early return.
- **Trait-based abstraction** — strict orphan rule, globally unique `(trait, type)` pairs, sealed traits, operator overloading via traits.
- **Seamless Java interop** — `import java.util.List;`, `safe { }` for Java exception boundaries, classpath-aware compilation.
- **Inline functions and reified generics** — `inline fn` with `reified` type parameters for runtime type access without reflection.
- **Modern syntax** — `fn`, `let` / `let mut`, `match`, `::` for enum variants, `.` for member access, `f"string interpolation"`.
- **JVM 21 baseline, 25 opt-in** — Valhalla value types gated behind `--target 25`.
- **Tooling** — LSP server (`valen-lsp`) with completions, hover, go-to-definition, diagnostics, and semantic highlighting. Code formatter (`valenfmt`).

## Install

### Script (Linux / macOS)

```sh
curl -fsSL https://raw.githubusercontent.com/NaruseNia/valen-lang/main/install.sh | bash
```

This installs `valenc` and `valen-lsp` to `~/.valen/bin`. Add it to your PATH:

```sh
export PATH="$HOME/.valen/bin:$PATH"
```

### From source

```sh
cargo install --path crates/valenc
cargo install --path crates/valen-lsp
```

### GitHub Release

Download pre-built binaries from [Releases](https://github.com/NaruseNia/valen-lang/releases). Available for Linux x64, macOS x64/arm64, and Windows x64.

## Usage

```sh
# Compile .vln files to .class
valenc compile src/main.vln -o out/

# Type check only (no codegen)
valenc check src/main.vln

# Run with Java
java -cp out/ com.example.Main

# Format source files
valenfmt src/main.vln
```

## Documentation

- [Language Specification](docs/LANGUAGE_SPEC.md) — formal language reference
- [User Guide](https://narusenia.github.io/valen-docs/) — tutorial-style introduction
- [Compiler Architecture](docs/guide/09-compiler-architecture.md) — internals for contributors

## License

[Apache License 2.0](LICENSE)
