//! HIR to JVM bytecode emitter.
//!
//! enum emission strategy (see docs/LANGUAGE_SPEC.md §6.3):
//! - `public sealed interface <Enum> permits <Enum>$<Variant>, ...`
//! - payload-bearing variant → `public static final record <Enum>$<Variant>(...) implements <Enum>`
//! - payload-less variant → `public static final class <Enum>$<Variant>` with an `INSTANCE` singleton
//!
//! Phase 0 spike must validate:
//! - Java 21 pattern `switch` exhaustiveness interop
//! - Jackson / Gson serialization round-trip
//! - `java.lang.reflect` class name resolution
//! - Gradle incremental compilation

pub mod class_emit;
pub mod enum_emit;

pub struct EmitTarget {
    pub jvm_version: JvmVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JvmVersion {
    Java21,
    Java25,
}

pub fn emit(_hir: &valen_hir::Hir, _target: EmitTarget) -> Vec<ClassFile> {
    todo!("walk HIR, emit class files via classfile crate")
}

pub struct ClassFile {
    pub internal_name: String,
    pub bytes: Vec<u8>,
}
