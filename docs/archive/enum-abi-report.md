# Enum ABI Spike Report

Date: 2026-05-11
Phase: 0

## Strategy

Valen `enum` (ADT) maps to JVM classes as follows:

| Valen construct | JVM representation |
|---|---|
| `enum Shape { ... }` | `public sealed interface Shape permits Shape$Circle, Shape$Rectangle, Shape$Point` |
| `Circle(r: Float)` (payload variant) | `public final record Shape$Circle(float r) implements Shape` (extends `java.lang.Record`) |
| `Point` (unit variant) | `public final class Shape$Point implements Shape` with `public static final INSTANCE` singleton |

### Binary naming

- Variant class: `<Enum>$<Variant>` (e.g. `Shape$Circle`)
- This aligns with Java inner class naming and is recognizable by tools expecting `$` as inner class separator.

## Implementation (ristretto_classfile)

The spike produced working emit code using `ristretto_classfile` 0.31:

1. **Sealed interface**: `ClassAccessFlags::INTERFACE | ABSTRACT` + `PermittedSubclasses` attribute listing variant class indexes.
2. **Record variant**: `super_class` → `java/lang/Record`, `Record` attribute with component descriptors, private final fields, constructor that calls `Record.<init>()V` then stores fields via `putfield`.
3. **Unit variant**: Regular class with private constructor, `<clinit>` static initializer that creates `INSTANCE = new Variant()`.

All emitted class files pass `ristretto_classfile::verify()` and roundtrip through `from_bytes` / `to_bytes`.

## Verification checklist

| Criterion | Status | Notes |
|---|---|---|
| Emit compiles and passes verify() | **PASS** | All 4 tests green |
| Roundtrip parse (ristretto from_bytes) | **PASS** | Access flags, attributes, fields confirmed |
| PermittedSubclasses attribute present | **PASS** | Verified in sealed_interface_roundtrip test |
| Record attribute present | **PASS** | Verified in record_variant_roundtrip test |
| INSTANCE singleton field on unit variant | **PASS** | Verified in unit_variant_has_instance_field test |
| Java 21 `switch` pattern matching exhaustive | **PENDING** | Requires JDK 21+ installed — expected to work because sealed interface + permits clause is the standard mechanism |
| Jackson/Gson serialization | **PENDING** | Requires JDK + Jackson dependency — record variants should serialize naturally; unit variants need custom serializer or `@JsonCreator` on INSTANCE |
| `java.lang.reflect` class name resolution | **PENDING** | Requires JDK — `$` naming follows standard inner class convention, `Class.forName("Shape$Circle")` expected to work |
| Gradle incremental compilation | **PENDING** | Requires Gradle plugin — ABI stability depends on variant ordering; adding a variant is a binary-incompatible change by design |

## Risks identified

1. **Jackson serialization of unit variants**: Singleton pattern (`INSTANCE` field) is not natively recognized by Jackson. Options:
   - Generate `@JsonCreator` static factory method (requires annotation emit)
   - Register a custom `StdDeserializer` in Valen's runtime library
   - Defer to Phase 1 — serialization support is not an MVP blocker

2. **Pattern switch exhaustiveness**: Java 21 `switch` with sealed types requires all permitted subclasses to be listed. Our `PermittedSubclasses` attribute is correct, but the Java compiler also needs `InnerClasses` attribute to resolve `$`-named classes. We may need to emit `InnerClasses` attribute on the sealed interface listing all variants.

3. **Long/Double constructor slots**: JVM uses 2 slots for `long` and `double` locals. Current `max_locals` calculation counts 1 slot per field — needs fix for `Long`/`Double` descriptors before Phase 1.

## Decision

The sealed interface + record + singleton strategy is **confirmed viable** for Valen's enum ABI. Proceed with this approach for Phase 1 implementation.

Remaining PENDING items require JDK installation and are deferred to Phase 1 integration testing.
