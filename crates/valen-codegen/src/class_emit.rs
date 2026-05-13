//! Minimal class emitter for early prototyping (single empty class with default `<init>`).

use ristretto_classfile::attributes::{Attribute, Instruction};
use ristretto_classfile::{
    ClassAccessFlags, ClassFile, ConstantPool, Method, MethodAccessFlags, JAVA_21,
};

use crate::ClassFileOutput;

/// Emit a minimal JVM class with only a default no-arg constructor.
pub fn emit_class(name: &str) -> Result<ClassFileOutput, ristretto_classfile::Error> {
    let mut cp = ConstantPool::default();

    let this_class = cp.add_class(name)?;
    let super_class = cp.add_class("java/lang/Object")?;

    let init_name = cp.add_utf8("<init>")?;
    let init_desc = cp.add_utf8("()V")?;
    let code_name = cp.add_utf8("Code")?;

    let super_init_ref = cp.add_method_ref(super_class, "<init>", "()V")?;

    let init_method = Method {
        access_flags: MethodAccessFlags::PUBLIC,
        name_index: init_name,
        descriptor_index: init_desc,
        attributes: vec![Attribute::Code {
            name_index: code_name,
            max_stack: 1,
            max_locals: 1,
            code: vec![
                Instruction::Aload_0,
                Instruction::Invokespecial(super_init_ref),
                Instruction::Return,
            ],
            exception_table: vec![],
            attributes: vec![],
        }],
    };

    let class_file = ClassFile {
        version: JAVA_21,
        constant_pool: cp,
        access_flags: ClassAccessFlags::PUBLIC | ClassAccessFlags::SUPER,
        this_class,
        super_class,
        methods: vec![init_method],
        ..Default::default()
    };

    class_file.verify()?;

    let mut bytes = Vec::new();
    class_file.to_bytes(&mut bytes)?;

    Ok(ClassFileOutput {
        internal_name: name.to_string(),
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_empty_class() {
        let result = emit_class("Foo").unwrap();
        assert_eq!(result.internal_name, "Foo");
        assert!(result.bytes.len() > 20);
        // Verify magic number (0xCAFEBABE)
        assert_eq!(&result.bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    }

    #[test]
    fn emit_roundtrip() {
        let result = emit_class("com/example/Bar").unwrap();
        let parsed = ClassFile::from_bytes(&result.bytes).unwrap();
        assert_eq!(parsed.class_name().unwrap(), "com/example/Bar");
        assert!(parsed.access_flags.contains(ClassAccessFlags::PUBLIC));
        assert_eq!(parsed.methods.len(), 1);
    }
}
