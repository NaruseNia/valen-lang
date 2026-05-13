//! Generates `equals`, `hashCode`, `toString`, and `copy` methods for data classes.

use crate::jvm_const::*;
use crate::jvm_ir::{ArithOp, JvmMethod, JvmMethodAccess, JvmMethodBody, JvmOp, JvmType};

/// Generates a structural `equals(Object)` method comparing all fields.
pub fn generate_equals(class_internal: &str, fields: &[(String, JvmType)]) -> JvmMethod {
    let obj = JvmType::Object(JVM_OBJECT.to_string());
    let cls = JvmType::Object(class_internal.to_string());
    let locals_before_cast = vec![cls.clone(), obj.clone()];
    let locals_after_cast = vec![cls.clone(), obj.clone(), cls.clone()];

    let mut ops = Vec::new();
    let mut label = 0u32;
    let label_false = {
        label += 1;
        label
    };
    let label_end = {
        label += 1;
        label
    };

    // if (this == other) return true
    ops.push(JvmOp::LoadThis);
    ops.push(JvmOp::LoadLocal(1, JvmType::Object(JVM_OBJECT.to_string())));
    ops.push(JvmOp::IfACmpNe({
        label += 1;
        label
    }));
    ops.push(JvmOp::PushInt(1));
    ops.push(JvmOp::Return(JvmType::Int));
    ops.push(JvmOp::Label(label)); // skip_identity
    ops.push(JvmOp::Frame {
        locals: locals_before_cast.clone(),
        stack: vec![],
    });

    // if (!(other instanceof ClassName)) return false
    ops.push(JvmOp::LoadLocal(1, JvmType::Object(JVM_OBJECT.to_string())));
    ops.push(JvmOp::Instanceof(class_internal.to_string()));
    ops.push(JvmOp::IfNe({
        label += 1;
        label
    }));
    ops.push(JvmOp::PushInt(0));
    ops.push(JvmOp::Return(JvmType::Int));
    ops.push(JvmOp::Label(label)); // is_instance
    ops.push(JvmOp::Frame {
        locals: locals_before_cast.clone(),
        stack: vec![],
    });

    // ClassName that = (ClassName) other
    let that_slot = 2u16;
    ops.push(JvmOp::LoadLocal(1, JvmType::Object(JVM_OBJECT.to_string())));
    ops.push(JvmOp::Checkcast(class_internal.to_string()));
    ops.push(JvmOp::StoreLocal(
        that_slot,
        JvmType::Object(class_internal.to_string()),
    ));

    for (fname, fty) in fields {
        ops.push(JvmOp::LoadThis);
        ops.push(JvmOp::GetField {
            owner: class_internal.to_string(),
            name: fname.clone(),
            descriptor: fty.clone(),
        });
        ops.push(JvmOp::LoadLocal(
            that_slot,
            JvmType::Object(class_internal.to_string()),
        ));
        ops.push(JvmOp::GetField {
            owner: class_internal.to_string(),
            name: fname.clone(),
            descriptor: fty.clone(),
        });

        match fty {
            JvmType::Float => {
                ops.push(JvmOp::InvokeStatic {
                    owner: JVM_FLOAT.to_string(),
                    name: COMPARE.to_string(),
                    params: vec![JvmType::Float, JvmType::Float],
                    ret: JvmType::Int,
                });
                ops.push(JvmOp::IfNe(label_false));
            }
            JvmType::Double => {
                ops.push(JvmOp::InvokeStatic {
                    owner: JVM_DOUBLE.to_string(),
                    name: COMPARE.to_string(),
                    params: vec![JvmType::Double, JvmType::Double],
                    ret: JvmType::Int,
                });
                ops.push(JvmOp::IfNe(label_false));
            }
            JvmType::Long => {
                ops.push(JvmOp::InvokeStatic {
                    owner: JVM_LONG.to_string(),
                    name: COMPARE.to_string(),
                    params: vec![JvmType::Long, JvmType::Long],
                    ret: JvmType::Int,
                });
                ops.push(JvmOp::IfNe(label_false));
            }
            JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean => {
                ops.push(JvmOp::IfICmpNe(label_false));
            }
            _ => {
                ops.push(JvmOp::InvokeStatic {
                    owner: JVM_OBJECTS.to_string(),
                    name: EQUALS.to_string(),
                    params: vec![
                        JvmType::Object(JVM_OBJECT.to_string()),
                        JvmType::Object(JVM_OBJECT.to_string()),
                    ],
                    ret: JvmType::Boolean,
                });
                ops.push(JvmOp::IfEq(label_false));
            }
        }
    }

    // return true
    ops.push(JvmOp::PushInt(1));
    ops.push(JvmOp::Goto(label_end));
    // return false
    ops.push(JvmOp::Label(label_false));
    ops.push(JvmOp::Frame {
        locals: locals_after_cast.clone(),
        stack: vec![],
    });
    ops.push(JvmOp::PushInt(0));
    ops.push(JvmOp::Label(label_end));
    ops.push(JvmOp::Frame {
        locals: locals_after_cast,
        stack: vec![JvmType::Int],
    });
    ops.push(JvmOp::Return(JvmType::Int));

    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: EQUALS.to_string(),
        params: vec![JvmType::Object(JVM_OBJECT.to_string())],
        return_type: JvmType::Boolean,
        body: Some(JvmMethodBody { max_locals: 3, ops }),
    }
}

/// Generates a `hashCode()` method using the 31-multiply-accumulate algorithm.
pub fn generate_hash_code(class_internal: &str, fields: &[(String, JvmType)]) -> JvmMethod {
    let mut ops = Vec::new();

    // int result = 1
    ops.push(JvmOp::PushInt(1));
    let result_slot = 1u16;
    ops.push(JvmOp::StoreLocal(result_slot, JvmType::Int));

    for (fname, fty) in fields {
        // result = 31 * result + hash(field)
        ops.push(JvmOp::PushInt(31));
        ops.push(JvmOp::LoadLocal(result_slot, JvmType::Int));
        ops.push(JvmOp::Arith(ArithOp::Mul, JvmType::Int));

        ops.push(JvmOp::LoadThis);
        ops.push(JvmOp::GetField {
            owner: class_internal.to_string(),
            name: fname.clone(),
            descriptor: fty.clone(),
        });

        let (hash_owner, hash_param) = match fty {
            JvmType::Int => (JVM_INTEGER, JvmType::Int),
            JvmType::Long => (JVM_LONG, JvmType::Long),
            JvmType::Float => (JVM_FLOAT, JvmType::Float),
            JvmType::Double => (JVM_DOUBLE, JvmType::Double),
            JvmType::Boolean => (JVM_BOOLEAN, JvmType::Boolean),
            JvmType::Char => (JVM_CHARACTER, JvmType::Char),
            JvmType::Byte => (JVM_BYTE, JvmType::Byte),
            JvmType::Short => (JVM_SHORT, JvmType::Short),
            _ => (JVM_OBJECTS, JvmType::Object(JVM_OBJECT.to_string())),
        };

        ops.push(JvmOp::InvokeStatic {
            owner: hash_owner.to_string(),
            name: HASH_CODE.to_string(),
            params: vec![hash_param],
            ret: JvmType::Int,
        });
        ops.push(JvmOp::Arith(ArithOp::Add, JvmType::Int));
        ops.push(JvmOp::StoreLocal(result_slot, JvmType::Int));
    }

    ops.push(JvmOp::LoadLocal(result_slot, JvmType::Int));
    ops.push(JvmOp::Return(JvmType::Int));

    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: HASH_CODE.to_string(),
        params: vec![],
        return_type: JvmType::Int,
        body: Some(JvmMethodBody { max_locals: 2, ops }),
    }
}

/// Generates a `toString()` method producing `ClassName(field=value, ...)` format.
pub fn generate_to_string(
    class_internal: &str,
    class_simple_name: &str,
    fields: &[(String, JvmType)],
) -> JvmMethod {
    let mut ops = Vec::new();

    // new StringBuilder("ClassName(")
    ops.push(JvmOp::New(JVM_STRING_BUILDER.to_string()));
    ops.push(JvmOp::Dup);
    ops.push(JvmOp::PushString(format!("{class_simple_name}(")));
    ops.push(JvmOp::InvokeSpecial {
        owner: JVM_STRING_BUILDER.to_string(),
        name: INIT.to_string(),
        params: vec![JvmType::Object(JVM_STRING.to_string())],
        ret: JvmType::Void,
    });

    for (i, (fname, fty)) in fields.iter().enumerate() {
        if i > 0 {
            ops.push(JvmOp::PushString(", ".to_string()));
            ops.push(JvmOp::InvokeVirtual {
                owner: JVM_STRING_BUILDER.to_string(),
                name: APPEND.to_string(),
                params: vec![JvmType::Object(JVM_STRING.to_string())],
                ret: JvmType::Object(JVM_STRING_BUILDER.to_string()),
            });
        }

        // "field="
        ops.push(JvmOp::PushString(format!("{fname}=")));
        ops.push(JvmOp::InvokeVirtual {
            owner: JVM_STRING_BUILDER.to_string(),
            name: APPEND.to_string(),
            params: vec![JvmType::Object(JVM_STRING.to_string())],
            ret: JvmType::Object(JVM_STRING_BUILDER.to_string()),
        });

        // this.field
        ops.push(JvmOp::LoadThis);
        ops.push(JvmOp::GetField {
            owner: class_internal.to_string(),
            name: fname.clone(),
            descriptor: fty.clone(),
        });

        let append_param = sb_append_type(fty);
        ops.push(JvmOp::InvokeVirtual {
            owner: JVM_STRING_BUILDER.to_string(),
            name: APPEND.to_string(),
            params: vec![append_param],
            ret: JvmType::Object(JVM_STRING_BUILDER.to_string()),
        });
    }

    // .append(")").toString()
    ops.push(JvmOp::PushString(")".to_string()));
    ops.push(JvmOp::InvokeVirtual {
        owner: JVM_STRING_BUILDER.to_string(),
        name: APPEND.to_string(),
        params: vec![JvmType::Object(JVM_STRING.to_string())],
        ret: JvmType::Object(JVM_STRING_BUILDER.to_string()),
    });
    ops.push(JvmOp::InvokeVirtual {
        owner: JVM_STRING_BUILDER.to_string(),
        name: TO_STRING.to_string(),
        params: vec![],
        ret: JvmType::Object(JVM_STRING.to_string()),
    });
    ops.push(JvmOp::Return(JvmType::Object(JVM_STRING.to_string())));

    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: TO_STRING.to_string(),
        params: vec![],
        return_type: JvmType::Object(JVM_STRING.to_string()),
        body: Some(JvmMethodBody { max_locals: 1, ops }),
    }
}

fn sb_append_type(ty: &JvmType) -> JvmType {
    match ty {
        JvmType::Int | JvmType::Byte | JvmType::Short => JvmType::Int,
        JvmType::Long => JvmType::Long,
        JvmType::Float => JvmType::Float,
        JvmType::Double => JvmType::Double,
        JvmType::Char => JvmType::Char,
        JvmType::Boolean => JvmType::Boolean,
        _ => JvmType::Object(JVM_OBJECT.to_string()),
    }
}

/// Generates a `copy(...)` method that constructs a new instance with the given field values.
pub fn generate_copy(class_internal: &str, fields: &[(String, JvmType)]) -> JvmMethod {
    let mut ops = Vec::new();

    // new ClassName(param1, param2, ...)
    ops.push(JvmOp::New(class_internal.to_string()));
    ops.push(JvmOp::Dup);

    let mut slot = 1u16;
    for (_fname, fty) in fields {
        ops.push(JvmOp::LoadLocal(slot, fty.clone()));
        slot += fty.slot_count();
    }

    ops.push(JvmOp::InvokeSpecial {
        owner: class_internal.to_string(),
        name: INIT.to_string(),
        params: fields.iter().map(|(_, ty)| ty.clone()).collect(),
        ret: JvmType::Void,
    });
    ops.push(JvmOp::Return(JvmType::Object(class_internal.to_string())));

    let max_locals = 1 + fields.iter().map(|(_, ty)| ty.slot_count()).sum::<u16>();

    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: "copy".to_string(),
        params: fields.iter().map(|(_, ty)| ty.clone()).collect(),
        return_type: JvmType::Object(class_internal.to_string()),
        body: Some(JvmMethodBody { max_locals, ops }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_has_correct_signature() {
        let m = generate_equals("Foo", &[("x".into(), JvmType::Int)]);
        assert_eq!(m.name, "equals");
        assert_eq!(m.params, vec![JvmType::Object(JVM_OBJECT.to_string())]);
        assert_eq!(m.return_type, JvmType::Boolean);
        assert!(m.body.is_some());
    }

    #[test]
    fn hash_code_has_correct_signature() {
        let m = generate_hash_code("Foo", &[("x".into(), JvmType::Int)]);
        assert_eq!(m.name, "hashCode");
        assert!(m.params.is_empty());
        assert_eq!(m.return_type, JvmType::Int);
    }

    #[test]
    fn to_string_has_correct_signature() {
        let m = generate_to_string("Foo", "Foo", &[("x".into(), JvmType::Int)]);
        assert_eq!(m.name, "toString");
        assert!(m.params.is_empty());
        assert_eq!(m.return_type, JvmType::Object(JVM_STRING.to_string()));
    }

    #[test]
    fn copy_has_correct_signature() {
        let fields = vec![("x".into(), JvmType::Float), ("y".into(), JvmType::Float)];
        let m = generate_copy("Point", &fields);
        assert_eq!(m.name, "copy");
        assert_eq!(m.params, vec![JvmType::Float, JvmType::Float]);
        assert_eq!(m.return_type, JvmType::Object("Point".to_string()));
        assert_eq!(m.body.as_ref().unwrap().max_locals, 3); // this + 2 floats
    }

    #[test]
    fn copy_wide_locals() {
        let fields = vec![("a".into(), JvmType::Long), ("b".into(), JvmType::Double)];
        let m = generate_copy("W", &fields);
        assert_eq!(m.body.as_ref().unwrap().max_locals, 5); // this(1) + long(2) + double(2)
    }
}
