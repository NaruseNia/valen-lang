//! Commonly used JVM class / method name constants.
//!
//! Centralises string literals that were previously scattered across
//! `lower.rs`, `expr.rs`, `data_class_methods.rs`, and `emit.rs`.

// -- class / type internal names --
pub const JVM_OBJECT: &str = "java/lang/Object";
pub const JVM_STRING: &str = "java/lang/String";
pub const JVM_STRING_BUILDER: &str = "java/lang/StringBuilder";
pub const JVM_RECORD: &str = "java/lang/Record";
pub const JVM_OBJECTS: &str = "java/util/Objects";
pub const JVM_FLOAT: &str = "java/lang/Float";
pub const JVM_DOUBLE: &str = "java/lang/Double";
pub const JVM_LONG: &str = "java/lang/Long";
pub const JVM_INTEGER: &str = "java/lang/Integer";
pub const JVM_BOOLEAN: &str = "java/lang/Boolean";
pub const JVM_CHARACTER: &str = "java/lang/Character";
pub const JVM_BYTE: &str = "java/lang/Byte";
pub const JVM_SHORT: &str = "java/lang/Short";
pub const JVM_UNSUPPORTED_OP: &str = "java/lang/UnsupportedOperationException";

// -- method names --
pub const INIT: &str = "<init>";
pub const CLINIT: &str = "<clinit>";
pub const EQUALS: &str = "equals";
pub const HASH_CODE: &str = "hashCode";
pub const TO_STRING: &str = "toString";
pub const APPEND: &str = "append";
pub const COMPARE: &str = "compare";
pub const INSTANCE: &str = "INSTANCE";
