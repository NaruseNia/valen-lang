//! Emit enum as `sealed interface` + (record | singleton class) per variant.
//!
//! See docs/LANGUAGE_SPEC.md §6.3 for the strategy.

pub fn emit_enum() {
    todo!("sealed interface + permits clause + per-variant record/singleton")
}

pub fn emit_payload_variant() {
    todo!("public static final record <Enum>$<Variant>(...) implements <Enum>")
}

pub fn emit_unit_variant() {
    // public static final class <Enum>$<Variant> implements <Enum> {
    //     public static final INSTANCE ...
    // }
    todo!("emit singleton class with INSTANCE field for payload-less variants")
}
