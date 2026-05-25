# Examples

Valen example projects. Each uses the Gradle plugin to compile `.vln` files to JVM bytecode.

## Prerequisites

- **Rust toolchain** (for building `valenc`)
- **Java 21** (Gradle plugin requires JDK 21; JDK 25 causes build failures)
- **Gradle plugin** built locally

## One-time setup

```bash
# 1. Build valenc and add to PATH
cargo build --release -p valenc
mkdir -p ~/.local/bin
ln -sf "$(pwd)/target/release/valenc" ~/.local/bin/valenc
export PATH="$HOME/.local/bin:$PATH"

# 2. Build and publish the Gradle plugin to local Maven
cd ../valen-gradle-plugin/plugin-build
JAVA_HOME=$(mise where java 21.0.2) ./gradlew :plugin:publishToMavenLocal
cd -
```

## Running an example

```bash
cd examples/type-safe-builder
JAVA_HOME=$(mise where java 21.0.2) ./gradlew compileValen
java -cp build/classes/valen/main builder.Main
```

### Expected output (type-safe-builder)

```
=== Type-safe builder ===
  built: submit-btn
  position: (builder.Meters@..., builder.Meters@...)
  size: builder.Meters@... x builder.Meters@...
  visible: true, opacity: 1.0
  border: solid builder.Pixels@...px rgb(0, 0, 0)
  alignment: centered

=== Hidden overlay ===
  overlay is hidden (opacity=0.5)

=== ref-mut counter ===
  counter after 2 + 3 increments: 5

=== Unsafe cast demo ===
  safe widening cast: 42 as Long = 42
  unsafe narrowing cast: 999L as Int = 999

builder demo complete
```

## Examples

| Project | Features demonstrated |
|---------|---------------------|
| `type-safe-builder` | newtype, data class, enum, match, f-string, ref-mut, unsafe/safe |
| `calculator` | ADT expression tree, Result/try operator, recursive evaluator |
| `ecs-system` | if-let, let-else, Java collections (HashMap, ArrayList) |
| `todo-app` | newtype IDs, derives, status transitions, filtering |

> **Note:** `calculator`, `ecs-system`, and `todo-app` require additional codegen fixes and may not run yet. `type-safe-builder` is fully functional.
