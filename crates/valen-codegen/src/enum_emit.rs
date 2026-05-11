use ristretto_classfile::attributes::{Attribute, Instruction, Record};
use ristretto_classfile::{
    BaseType, ClassAccessFlags, ClassFile, ConstantPool, Field, FieldAccessFlags, FieldType,
    JavaString, Method, MethodAccessFlags, JAVA_21,
};

use crate::ClassFileOutput;

pub struct EnumVariantDef {
    pub name: String,
    pub fields: Vec<(String, String)>,
}

pub fn emit_enum(
    enum_name: &str,
    variants: &[EnumVariantDef],
) -> Result<Vec<ClassFileOutput>, ristretto_classfile::Error> {
    let mut outputs = Vec::new();
    let variant_names: Vec<String> = variants
        .iter()
        .map(|v| format!("{}${}", enum_name, v.name))
        .collect();

    outputs.push(emit_sealed_interface(enum_name, &variant_names)?);

    for variant in variants {
        let internal = format!("{}${}", enum_name, variant.name);
        if variant.fields.is_empty() {
            outputs.push(emit_unit_variant(&internal, enum_name)?);
        } else {
            outputs.push(emit_record_variant(&internal, enum_name, &variant.fields)?);
        }
    }

    Ok(outputs)
}

fn emit_sealed_interface(
    name: &str,
    permitted: &[String],
) -> Result<ClassFileOutput, ristretto_classfile::Error> {
    let mut cp = ConstantPool::default();
    let this_class = cp.add_class(name)?;
    let object_class = cp.add_class("java/lang/Object")?;

    let permitted_indexes: Vec<u16> = permitted
        .iter()
        .map(|p| cp.add_class(p.as_str()))
        .collect::<Result<_, _>>()?;

    let ps_name = cp.add_utf8("PermittedSubclasses")?;

    let cf = ClassFile {
        version: JAVA_21,
        constant_pool: cp,
        access_flags: ClassAccessFlags::PUBLIC
            | ClassAccessFlags::INTERFACE
            | ClassAccessFlags::ABSTRACT,
        this_class,
        super_class: object_class,
        attributes: vec![Attribute::PermittedSubclasses {
            name_index: ps_name,
            class_indexes: permitted_indexes,
        }],
        ..Default::default()
    };

    cf.verify()?;
    let mut bytes = Vec::new();
    cf.to_bytes(&mut bytes)?;
    Ok(ClassFileOutput {
        internal_name: name.to_string(),
        bytes,
    })
}

fn emit_record_variant(
    internal_name: &str,
    interface_name: &str,
    fields: &[(String, String)],
) -> Result<ClassFileOutput, ristretto_classfile::Error> {
    let mut cp = ConstantPool::default();
    let this_class = cp.add_class(internal_name)?;
    let record_class = cp.add_class("java/lang/Record")?;
    let iface_class = cp.add_class(interface_name)?;
    let init_name = cp.add_utf8("<init>")?;
    let code_name = cp.add_utf8("Code")?;
    let record_attr_name = cp.add_utf8("Record")?;

    let record_init_ref = cp.add_method_ref(record_class, "<init>", "()V")?;

    let mut ctor_desc = String::from("(");
    let mut record_components = Vec::new();
    let mut field_defs = Vec::new();

    for (fname, fdesc) in fields {
        ctor_desc.push_str(fdesc);

        let fname_idx = cp.add_utf8(fname)?;
        let fdesc_idx = cp.add_utf8(fdesc)?;

        record_components.push(Record {
            name_index: fname_idx,
            descriptor_index: fdesc_idx,
            attributes: vec![],
        });

        let ft = descriptor_to_field_type(fdesc);
        field_defs.push(Field {
            access_flags: FieldAccessFlags::PRIVATE | FieldAccessFlags::FINAL,
            name_index: fname_idx,
            descriptor_index: fdesc_idx,
            field_type: ft,
            attributes: vec![],
        });
    }
    ctor_desc.push(')');
    ctor_desc.push('V');

    let ctor_desc_idx = cp.add_utf8(&ctor_desc)?;

    // max_locals = 1 (this) + number of fields
    let max_locals = (1 + fields.len()) as u16;

    let mut ctor_code = vec![Instruction::Aload_0];

    ctor_code.push(Instruction::Invokespecial(record_init_ref));

    for (i, (fname, fdesc)) in fields.iter().enumerate() {
        let slot = (i + 1) as u8;
        match fdesc.as_str() {
            "I" | "Z" | "B" | "S" | "C" => ctor_code.push(Instruction::Iload(slot)),
            "J" => ctor_code.push(Instruction::Lload(slot)),
            "F" => ctor_code.push(Instruction::Fload(slot)),
            "D" => ctor_code.push(Instruction::Dload(slot)),
            _ => ctor_code.push(Instruction::Aload(slot)),
        }
        ctor_code.push(Instruction::Aload_0);
        // swap receiver and value
        ctor_code.push(Instruction::Swap);
        let field_ref = cp.add_field_ref(this_class, fname, fdesc)?;
        ctor_code.push(Instruction::Putfield(field_ref));
    }

    ctor_code.push(Instruction::Return);

    let init_method = Method {
        access_flags: MethodAccessFlags::PUBLIC,
        name_index: init_name,
        descriptor_index: ctor_desc_idx,
        attributes: vec![Attribute::Code {
            name_index: code_name,
            max_stack: 3,
            max_locals,
            code: ctor_code,
            exception_table: vec![],
            attributes: vec![],
        }],
    };

    let cf = ClassFile {
        version: JAVA_21,
        constant_pool: cp,
        access_flags: ClassAccessFlags::PUBLIC | ClassAccessFlags::FINAL | ClassAccessFlags::SUPER,
        this_class,
        super_class: record_class,
        interfaces: vec![iface_class],
        fields: field_defs,
        methods: vec![init_method],
        attributes: vec![Attribute::Record {
            name_index: record_attr_name,
            records: record_components,
        }],
        ..Default::default()
    };

    cf.verify()?;
    let mut bytes = Vec::new();
    cf.to_bytes(&mut bytes)?;
    Ok(ClassFileOutput {
        internal_name: internal_name.to_string(),
        bytes,
    })
}

fn emit_unit_variant(
    internal_name: &str,
    interface_name: &str,
) -> Result<ClassFileOutput, ristretto_classfile::Error> {
    let mut cp = ConstantPool::default();
    let this_class = cp.add_class(internal_name)?;
    let object_class = cp.add_class("java/lang/Object")?;
    let iface_class = cp.add_class(interface_name)?;

    let init_name = cp.add_utf8("<init>")?;
    let init_desc = cp.add_utf8("()V")?;
    let code_name = cp.add_utf8("Code")?;
    let instance_name = cp.add_utf8("INSTANCE")?;
    let instance_desc = cp.add_utf8(format!("L{internal_name};"))?;

    let object_init = cp.add_method_ref(object_class, "<init>", "()V")?;

    let init_method = Method {
        access_flags: MethodAccessFlags::PRIVATE,
        name_index: init_name,
        descriptor_index: init_desc,
        attributes: vec![Attribute::Code {
            name_index: code_name,
            max_stack: 1,
            max_locals: 1,
            code: vec![
                Instruction::Aload_0,
                Instruction::Invokespecial(object_init),
                Instruction::Return,
            ],
            exception_table: vec![],
            attributes: vec![],
        }],
    };

    let clinit_name = cp.add_utf8("<clinit>")?;
    let clinit_desc = cp.add_utf8("()V")?;
    let this_init = cp.add_method_ref(this_class, "<init>", "()V")?;
    let instance_field_ref =
        cp.add_field_ref(this_class, "INSTANCE", &format!("L{internal_name};"))?;

    let clinit_method = Method {
        access_flags: MethodAccessFlags::STATIC,
        name_index: clinit_name,
        descriptor_index: clinit_desc,
        attributes: vec![Attribute::Code {
            name_index: code_name,
            max_stack: 2,
            max_locals: 0,
            code: vec![
                Instruction::New(this_class),
                Instruction::Dup,
                Instruction::Invokespecial(this_init),
                Instruction::Putstatic(instance_field_ref),
                Instruction::Return,
            ],
            exception_table: vec![],
            attributes: vec![],
        }],
    };

    let instance_field = Field {
        access_flags: FieldAccessFlags::PUBLIC | FieldAccessFlags::STATIC | FieldAccessFlags::FINAL,
        name_index: instance_name,
        descriptor_index: instance_desc,
        field_type: FieldType::Object(JavaString::from(internal_name)),
        attributes: vec![],
    };

    let cf = ClassFile {
        version: JAVA_21,
        constant_pool: cp,
        access_flags: ClassAccessFlags::PUBLIC | ClassAccessFlags::FINAL | ClassAccessFlags::SUPER,
        this_class,
        super_class: object_class,
        interfaces: vec![iface_class],
        fields: vec![instance_field],
        methods: vec![init_method, clinit_method],
        ..Default::default()
    };

    cf.verify()?;
    let mut bytes = Vec::new();
    cf.to_bytes(&mut bytes)?;
    Ok(ClassFileOutput {
        internal_name: internal_name.to_string(),
        bytes,
    })
}

fn descriptor_to_field_type(desc: &str) -> FieldType {
    match desc {
        "I" => FieldType::Base(BaseType::Int),
        "J" => FieldType::Base(BaseType::Long),
        "F" => FieldType::Base(BaseType::Float),
        "D" => FieldType::Base(BaseType::Double),
        "Z" => FieldType::Base(BaseType::Boolean),
        "B" => FieldType::Base(BaseType::Byte),
        "S" => FieldType::Base(BaseType::Short),
        "C" => FieldType::Base(BaseType::Char),
        s if s.starts_with('L') && s.ends_with(';') => {
            FieldType::Object(JavaString::from(&s[1..s.len() - 1]))
        }
        _ => FieldType::Object(JavaString::from(desc)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_shape_enum() {
        let variants = vec![
            EnumVariantDef {
                name: "Circle".to_string(),
                fields: vec![("r".to_string(), "F".to_string())],
            },
            EnumVariantDef {
                name: "Rectangle".to_string(),
                fields: vec![
                    ("w".to_string(), "F".to_string()),
                    ("h".to_string(), "F".to_string()),
                ],
            },
            EnumVariantDef {
                name: "Point".to_string(),
                fields: vec![],
            },
        ];

        let outputs = emit_enum("Shape", &variants).unwrap();
        assert_eq!(outputs.len(), 4);
        assert_eq!(outputs[0].internal_name, "Shape");
        assert_eq!(outputs[1].internal_name, "Shape$Circle");
        assert_eq!(outputs[2].internal_name, "Shape$Rectangle");
        assert_eq!(outputs[3].internal_name, "Shape$Point");

        for output in &outputs {
            assert_eq!(&output.bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        }
    }

    #[test]
    fn sealed_interface_roundtrip() {
        let outputs = emit_enum(
            "Shape",
            &[EnumVariantDef {
                name: "Point".to_string(),
                fields: vec![],
            }],
        )
        .unwrap();

        let iface = ClassFile::from_bytes(&outputs[0].bytes).unwrap();
        assert_eq!(iface.class_name().unwrap(), "Shape");
        assert!(iface
            .access_flags
            .contains(ClassAccessFlags::INTERFACE | ClassAccessFlags::ABSTRACT));

        let has_permitted = iface
            .attributes
            .iter()
            .any(|a| matches!(a, Attribute::PermittedSubclasses { .. }));
        assert!(has_permitted);
    }

    #[test]
    fn record_variant_roundtrip() {
        let outputs = emit_enum(
            "Color",
            &[EnumVariantDef {
                name: "Rgb".to_string(),
                fields: vec![
                    ("r".to_string(), "I".to_string()),
                    ("g".to_string(), "I".to_string()),
                    ("b".to_string(), "I".to_string()),
                ],
            }],
        )
        .unwrap();

        let record = ClassFile::from_bytes(&outputs[1].bytes).unwrap();
        assert_eq!(record.class_name().unwrap(), "Color$Rgb");
        assert!(record.access_flags.contains(ClassAccessFlags::FINAL));
        assert_eq!(record.fields.len(), 3);

        let has_record_attr = record
            .attributes
            .iter()
            .any(|a| matches!(a, Attribute::Record { .. }));
        assert!(has_record_attr);
    }

    #[test]
    fn unit_variant_has_instance_field() {
        let outputs = emit_enum(
            "Option",
            &[EnumVariantDef {
                name: "None".to_string(),
                fields: vec![],
            }],
        )
        .unwrap();

        let unit = ClassFile::from_bytes(&outputs[1].bytes).unwrap();
        assert_eq!(unit.class_name().unwrap(), "Option$None");
        assert_eq!(unit.fields.len(), 1);
        assert!(unit.fields[0].access_flags.contains(
            FieldAccessFlags::PUBLIC | FieldAccessFlags::STATIC | FieldAccessFlags::FINAL
        ));
        assert_eq!(unit.methods.len(), 2);
    }
}
