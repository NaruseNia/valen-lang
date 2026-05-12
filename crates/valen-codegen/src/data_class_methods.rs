use crate::jvm_ir::{ArithOp, JvmMethod, JvmMethodAccess, JvmMethodBody, JvmOp, JvmType};

pub fn generate_equals(class_internal: &str, fields: &[(String, JvmType)]) -> JvmMethod {
    let obj = JvmType::Object("java/lang/Object".to_string());
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
    ops.push(JvmOp::LoadLocal(
        1,
        JvmType::Object("java/lang/Object".to_string()),
    ));
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
    ops.push(JvmOp::LoadLocal(
        1,
        JvmType::Object("java/lang/Object".to_string()),
    ));
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
    ops.push(JvmOp::LoadLocal(
        1,
        JvmType::Object("java/lang/Object".to_string()),
    ));
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
                    owner: "java/lang/Float".to_string(),
                    name: "compare".to_string(),
                    params: vec![JvmType::Float, JvmType::Float],
                    ret: JvmType::Int,
                });
                ops.push(JvmOp::IfNe(label_false));
            }
            JvmType::Double => {
                ops.push(JvmOp::InvokeStatic {
                    owner: "java/lang/Double".to_string(),
                    name: "compare".to_string(),
                    params: vec![JvmType::Double, JvmType::Double],
                    ret: JvmType::Int,
                });
                ops.push(JvmOp::IfNe(label_false));
            }
            JvmType::Long => {
                ops.push(JvmOp::InvokeStatic {
                    owner: "java/lang/Long".to_string(),
                    name: "compare".to_string(),
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
                    owner: "java/util/Objects".to_string(),
                    name: "equals".to_string(),
                    params: vec![
                        JvmType::Object("java/lang/Object".to_string()),
                        JvmType::Object("java/lang/Object".to_string()),
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
        name: "equals".to_string(),
        params: vec![JvmType::Object("java/lang/Object".to_string())],
        return_type: JvmType::Boolean,
        body: Some(JvmMethodBody { max_locals: 3, ops }),
    }
}

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
            JvmType::Int => ("java/lang/Integer", JvmType::Int),
            JvmType::Long => ("java/lang/Long", JvmType::Long),
            JvmType::Float => ("java/lang/Float", JvmType::Float),
            JvmType::Double => ("java/lang/Double", JvmType::Double),
            JvmType::Boolean => ("java/lang/Boolean", JvmType::Boolean),
            JvmType::Char => ("java/lang/Character", JvmType::Char),
            JvmType::Byte => ("java/lang/Byte", JvmType::Byte),
            JvmType::Short => ("java/lang/Short", JvmType::Short),
            _ => (
                "java/util/Objects",
                JvmType::Object("java/lang/Object".to_string()),
            ),
        };

        ops.push(JvmOp::InvokeStatic {
            owner: hash_owner.to_string(),
            name: "hashCode".to_string(),
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
        name: "hashCode".to_string(),
        params: vec![],
        return_type: JvmType::Int,
        body: Some(JvmMethodBody { max_locals: 2, ops }),
    }
}

pub fn generate_to_string(
    class_internal: &str,
    class_simple_name: &str,
    fields: &[(String, JvmType)],
) -> JvmMethod {
    let sb = "java/lang/StringBuilder";
    let mut ops = Vec::new();

    // new StringBuilder("ClassName(")
    ops.push(JvmOp::New(sb.to_string()));
    ops.push(JvmOp::Dup);
    ops.push(JvmOp::PushString(format!("{class_simple_name}(")));
    ops.push(JvmOp::InvokeSpecial {
        owner: sb.to_string(),
        name: "<init>".to_string(),
        params: vec![JvmType::Object("java/lang/String".to_string())],
        ret: JvmType::Void,
    });

    for (i, (fname, fty)) in fields.iter().enumerate() {
        if i > 0 {
            ops.push(JvmOp::PushString(", ".to_string()));
            ops.push(JvmOp::InvokeVirtual {
                owner: sb.to_string(),
                name: "append".to_string(),
                params: vec![JvmType::Object("java/lang/String".to_string())],
                ret: JvmType::Object(sb.to_string()),
            });
        }

        // "field="
        ops.push(JvmOp::PushString(format!("{fname}=")));
        ops.push(JvmOp::InvokeVirtual {
            owner: sb.to_string(),
            name: "append".to_string(),
            params: vec![JvmType::Object("java/lang/String".to_string())],
            ret: JvmType::Object(sb.to_string()),
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
            owner: sb.to_string(),
            name: "append".to_string(),
            params: vec![append_param],
            ret: JvmType::Object(sb.to_string()),
        });
    }

    // .append(")").toString()
    ops.push(JvmOp::PushString(")".to_string()));
    ops.push(JvmOp::InvokeVirtual {
        owner: sb.to_string(),
        name: "append".to_string(),
        params: vec![JvmType::Object("java/lang/String".to_string())],
        ret: JvmType::Object(sb.to_string()),
    });
    ops.push(JvmOp::InvokeVirtual {
        owner: sb.to_string(),
        name: "toString".to_string(),
        params: vec![],
        ret: JvmType::Object("java/lang/String".to_string()),
    });
    ops.push(JvmOp::Return(JvmType::Object(
        "java/lang/String".to_string(),
    )));

    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: "toString".to_string(),
        params: vec![],
        return_type: JvmType::Object("java/lang/String".to_string()),
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
        _ => JvmType::Object("java/lang/Object".to_string()),
    }
}

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
        name: "<init>".to_string(),
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
        assert_eq!(
            m.params,
            vec![JvmType::Object("java/lang/Object".to_string())]
        );
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
        assert_eq!(
            m.return_type,
            JvmType::Object("java/lang/String".to_string())
        );
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
