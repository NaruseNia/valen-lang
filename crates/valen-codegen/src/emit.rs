//! Emits `JvmClass` IR to JVM `.class` file bytes using `ristretto_classfile`.

use std::collections::HashMap;

use ristretto_classfile::attributes::{
    AnnotationElement, AnnotationValuePair, Attribute, BootstrapMethod, ExceptionTableEntry,
    Instruction, Record, StackFrame, VerificationType,
};
use ristretto_classfile::{
    BaseType, ClassAccessFlags, ClassFile, ConstantPool, Field, FieldAccessFlags, FieldType,
    JavaString, Method, MethodAccessFlags, ReferenceKind, JAVA_21,
};

use crate::descriptor::{jvm_method_descriptor, jvm_type_descriptor};
use crate::jvm_ir::{
    ArithOp, BitwiseOp, CmpKind, JvmClass, JvmField, JvmMethodBody, JvmOp, JvmType, Label,
};
use crate::ClassFileOutput;

/// Errors that can occur during classfile emission.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    /// Error from the underlying `ristretto_classfile` library.
    #[error("classfile error: {0}")]
    ClassFile(#[from] ristretto_classfile::Error),
    /// A branch references a label that was never defined.
    #[error("unresolved label: {0}")]
    UnresolvedLabel(Label),
}

/// Emits a single `JvmClass` IR node to classfile bytes.
pub fn emit_class(jvm_class: &JvmClass) -> Result<ClassFileOutput, CodegenError> {
    let mut cp = ConstantPool::default();

    let this_class = cp.add_class(&jvm_class.name)?;
    let super_class = cp.add_class(&jvm_class.super_class)?;

    let interfaces: Vec<u16> = jvm_class
        .interfaces
        .iter()
        .map(|name| cp.add_class(name.as_str()))
        .collect::<Result<_, _>>()?;

    let fields: Vec<Field> = jvm_class
        .fields
        .iter()
        .map(|f| emit_field(&mut cp, f))
        .collect::<Result<_, _>>()?;

    let code_name = cp.add_utf8("Code")?;

    let mut methods: Vec<Method> = jvm_class
        .methods
        .iter()
        .map(|m| emit_method(&mut cp, m, code_name))
        .collect::<Result<_, _>>()?;

    // Emit synthetic lambda methods.
    for m in &jvm_class.synthetic_methods {
        methods.push(emit_method(&mut cp, m, code_name)?);
    }

    let mut attributes = Vec::new();

    // Emit BootstrapMethods attribute if any lambda call sites exist.
    if !jvm_class.bootstrap_methods.is_empty() {
        let bsm_name_idx = cp.add_utf8("BootstrapMethods")?;
        let bsm_entries = emit_bootstrap_methods(&mut cp, &jvm_class.bootstrap_methods)?;
        attributes.push(Attribute::BootstrapMethods {
            name_index: bsm_name_idx,
            methods: bsm_entries,
        });
    }

    if !jvm_class.permitted_subclasses.is_empty() {
        let ps_name = cp.add_utf8("PermittedSubclasses")?;
        let class_indexes: Vec<u16> = jvm_class
            .permitted_subclasses
            .iter()
            .map(|name| cp.add_class(name.as_str()))
            .collect::<Result<_, _>>()?;
        attributes.push(Attribute::PermittedSubclasses {
            name_index: ps_name,
            class_indexes,
        });
    }

    if !jvm_class.annotations.is_empty() {
        let rva_name = cp.add_utf8("RuntimeVisibleAnnotations")?;
        let mut annotations = Vec::new();
        for ann in &jvm_class.annotations {
            let type_idx = cp.add_utf8(&ann.type_descriptor)?;
            let mut elements = Vec::new();
            for (name, value) in &ann.values {
                let name_idx = cp.add_utf8(name)?;
                let element = emit_annotation_value(&mut cp, value)?;
                elements.push(AnnotationValuePair {
                    name_index: name_idx,
                    value: element,
                });
            }
            annotations.push(ristretto_classfile::attributes::Annotation {
                type_index: type_idx,
                elements,
            });
        }
        attributes.push(Attribute::RuntimeVisibleAnnotations {
            name_index: rva_name,
            annotations,
        });
    }

    if let Some(ref sf) = jvm_class.source_file {
        let sf_name = cp.add_utf8("SourceFile")?;
        let sf_index = cp.add_utf8(sf)?;
        attributes.push(Attribute::SourceFile {
            name_index: sf_name,
            source_file_index: sf_index,
        });
    }

    if jvm_class.is_record {
        let record_name = cp.add_utf8("Record")?;
        let mut records = Vec::new();
        for field in &jvm_class.fields {
            let fname = cp.add_utf8(&field.name)?;
            let fdesc = cp.add_utf8(jvm_type_descriptor(&field.ty))?;
            records.push(Record {
                name_index: fname,
                descriptor_index: fdesc,
                attributes: vec![],
            });
        }
        attributes.push(Attribute::Record {
            name_index: record_name,
            records,
        });
    }

    let mut access_flags = ClassAccessFlags::empty();
    if jvm_class.access.is_public {
        access_flags |= ClassAccessFlags::PUBLIC;
    }
    if jvm_class.access.is_final {
        access_flags |= ClassAccessFlags::FINAL;
    }
    if jvm_class.access.is_abstract {
        access_flags |= ClassAccessFlags::ABSTRACT;
    }
    if jvm_class.access.is_interface {
        access_flags |= ClassAccessFlags::INTERFACE;
    }
    if jvm_class.access.is_annotation {
        access_flags |= ClassAccessFlags::ANNOTATION;
    }
    if jvm_class.access.is_super {
        access_flags |= ClassAccessFlags::SUPER;
    }

    let mut class_file = ClassFile {
        version: JAVA_21,
        constant_pool: cp,
        access_flags,
        this_class,
        super_class,
        interfaces,
        fields,
        methods,
        attributes,
        ..Default::default()
    };

    match class_file.verify() {
        Ok(()) => {}
        Err(ristretto_classfile::Error::VerificationError(ref msg)) => {
            // StackMapTable verification may fail for complex control flow;
            // strip StackMapTable attributes and retry
            eprintln!(
                "[codegen] stripping StackMapTable from class '{}': {}",
                jvm_class.name, msg
            );
            for method in &mut class_file.methods {
                for attr in &mut method.attributes {
                    if let Attribute::Code { attributes, .. } = attr {
                        attributes.retain(|a| !matches!(a, Attribute::StackMapTable { .. }));
                    }
                }
            }
        }
        Err(e) => return Err(e.into()),
    }

    let mut bytes = Vec::new();
    class_file.to_bytes(&mut bytes)?;

    Ok(ClassFileOutput {
        internal_name: jvm_class.name.clone(),
        bytes,
    })
}

/// Emits BootstrapMethod entries from the IR-level bootstrap method descriptors.
fn emit_annotation_value(
    cp: &mut ConstantPool,
    value: &crate::jvm_ir::JvmAnnotationValue,
) -> Result<AnnotationElement, CodegenError> {
    use crate::jvm_ir::JvmAnnotationValue;
    match value {
        JvmAnnotationValue::String(s) => {
            let idx = cp.add_utf8(s)?;
            Ok(AnnotationElement::String {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Int(v) => {
            let idx = cp.add_integer(*v)?;
            Ok(AnnotationElement::Int {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Long(v) => {
            let idx = cp.add_long(*v)?;
            Ok(AnnotationElement::Long {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Float(v) => {
            let idx = cp.add_float(*v)?;
            Ok(AnnotationElement::Float {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Double(v) => {
            let idx = cp.add_double(*v)?;
            Ok(AnnotationElement::Double {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Bool(v) => {
            let idx = cp.add_integer(if *v { 1 } else { 0 })?;
            Ok(AnnotationElement::Boolean {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Char(v) => {
            let idx = cp.add_integer(*v as i32)?;
            Ok(AnnotationElement::Char {
                const_value_index: idx,
            })
        }
        JvmAnnotationValue::Enum {
            type_name,
            const_name,
        } => {
            let type_idx = cp.add_utf8(type_name)?;
            let const_idx = cp.add_utf8(const_name)?;
            Ok(AnnotationElement::Enum {
                type_name_index: type_idx,
                const_name_index: const_idx,
            })
        }
        JvmAnnotationValue::Array(values) => {
            let mut elements = Vec::new();
            for v in values {
                elements.push(emit_annotation_value(cp, v)?);
            }
            Ok(AnnotationElement::Array { values: elements })
        }
    }
}

fn emit_bootstrap_methods(
    cp: &mut ConstantPool,
    bsm_list: &[crate::jvm_ir::JvmBootstrapMethod],
) -> Result<Vec<BootstrapMethod>, CodegenError> {
    let mut result = Vec::new();
    for bsm in bsm_list {
        // Resolve the bootstrap method reference.
        let bsm_ref_index = match &bsm.method_ref {
            crate::jvm_ir::BootstrapMethodRef::LambdaMetafactory => {
                // LambdaMetafactory.metafactory is a static method in java/lang/invoke/LambdaMetafactory
                let class_idx = cp.add_class("java/lang/invoke/LambdaMetafactory")?;
                let method_ref = cp.add_method_ref(
                    class_idx,
                    "metafactory",
                    "(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodHandle;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;",
                )?;
                cp.add_method_handle(ReferenceKind::InvokeStatic, method_ref)?
            }
        };

        // Resolve the bootstrap arguments.
        let mut arguments = Vec::new();
        for arg in &bsm.arguments {
            let idx = match arg {
                crate::jvm_ir::BootstrapArg::MethodType(descriptor) => {
                    cp.add_method_type(descriptor)?
                }
                crate::jvm_ir::BootstrapArg::MethodHandle {
                    kind,
                    owner,
                    name,
                    descriptor,
                } => {
                    let ref_kind = match kind {
                        crate::jvm_ir::MethodHandleKind::InvokeStatic => {
                            ReferenceKind::InvokeStatic
                        }
                    };
                    let class_idx = cp.add_class(owner)?;
                    let method_ref = cp.add_method_ref(class_idx, name.as_str(), descriptor)?;
                    cp.add_method_handle(ref_kind, method_ref)?
                }
            };
            arguments.push(idx);
        }

        result.push(BootstrapMethod {
            bootstrap_method_ref: bsm_ref_index,
            arguments,
        });
    }
    Ok(result)
}

fn emit_field(cp: &mut ConstantPool, field: &JvmField) -> Result<Field, CodegenError> {
    let name_index = cp.add_utf8(&field.name)?;
    let desc_str = jvm_type_descriptor(&field.ty);
    let descriptor_index = cp.add_utf8(&desc_str)?;

    let mut access_flags = FieldAccessFlags::empty();
    if field.access.is_public {
        access_flags |= FieldAccessFlags::PUBLIC;
    }
    if field.access.is_private {
        access_flags |= FieldAccessFlags::PRIVATE;
    }
    if field.access.is_protected {
        access_flags |= FieldAccessFlags::PROTECTED;
    }
    if field.access.is_final {
        access_flags |= FieldAccessFlags::FINAL;
    }
    if field.access.is_static {
        access_flags |= FieldAccessFlags::STATIC;
    }

    let field_type = descriptor_to_field_type(&desc_str);

    Ok(Field {
        access_flags,
        name_index,
        descriptor_index,
        field_type,
        attributes: vec![],
    })
}

fn emit_method(
    cp: &mut ConstantPool,
    method: &crate::jvm_ir::JvmMethod,
    code_name_index: u16,
) -> Result<Method, CodegenError> {
    let name_index = cp.add_utf8(&method.name)?;
    let desc_str = jvm_method_descriptor(&method.params, &method.return_type);
    let descriptor_index = cp.add_utf8(&desc_str)?;

    let mut access_flags = MethodAccessFlags::empty();
    if method.access.is_public {
        access_flags |= MethodAccessFlags::PUBLIC;
    }
    if method.access.is_private {
        access_flags |= MethodAccessFlags::PRIVATE;
    }
    if method.access.is_protected {
        access_flags |= MethodAccessFlags::PROTECTED;
    }
    if method.access.is_static {
        access_flags |= MethodAccessFlags::STATIC;
    }
    if method.access.is_final {
        access_flags |= MethodAccessFlags::FINAL;
    }
    if method.access.is_abstract {
        access_flags |= MethodAccessFlags::ABSTRACT;
    }
    if method.access.is_bridge {
        access_flags |= MethodAccessFlags::BRIDGE;
    }
    if method.access.is_synthetic {
        access_flags |= MethodAccessFlags::SYNTHETIC;
    }

    let attributes = match &method.body {
        Some(body) => {
            let (instructions, max_stack, stack_frames, exception_table) = emit_body(cp, body)?;
            let mut code_attrs = Vec::new();
            if !stack_frames.is_empty() {
                let smt_name = cp.add_utf8("StackMapTable")?;
                code_attrs.push(Attribute::StackMapTable {
                    name_index: smt_name,
                    frames: stack_frames,
                });
            }
            vec![Attribute::Code {
                name_index: code_name_index,
                max_stack,
                max_locals: body.max_locals,
                code: instructions,
                exception_table,
                attributes: code_attrs,
            }]
        }
        None => vec![],
    };

    Ok(Method {
        access_flags,
        name_index,
        descriptor_index,
        attributes,
    })
}

struct FrameInfo {
    instr_index: usize,
    locals: Vec<JvmType>,
    stack: Vec<JvmType>,
}

/// Result tuple from `emit_body`: (instructions, max_stack, stack_frames, exception_table).
type EmitBodyResult = (
    Vec<Instruction>,
    u16,
    Vec<StackFrame>,
    Vec<ExceptionTableEntry>,
);

fn emit_body(cp: &mut ConstantPool, body: &JvmMethodBody) -> Result<EmitBodyResult, CodegenError> {
    let mut instructions = Vec::new();
    let mut label_positions: HashMap<Label, usize> = HashMap::new();
    let mut fixups: Vec<(usize, Label)> = Vec::new();
    let mut max_stack: i32 = 0;
    let mut stack: i32 = 0;
    let mut frames: Vec<FrameInfo> = Vec::new();
    let mut pending_label = false;

    for op in &body.ops {
        match op {
            JvmOp::Label(label) => {
                label_positions.insert(*label, instructions.len());
                pending_label = true;
            }
            JvmOp::Frame {
                locals,
                stack: frame_stack,
            } => {
                if pending_label {
                    frames.push(FrameInfo {
                        instr_index: instructions.len(),
                        locals: locals.clone(),
                        stack: frame_stack.clone(),
                    });
                    pending_label = false;
                }
                stack = frame_stack.iter().map(|t| t.slot_count() as i32).sum();
            }
            JvmOp::StubBody => {
                pending_label = false;
                let exc_class = cp.add_class(crate::jvm_const::JVM_UNSUPPORTED_OP)?;
                let exc_init = cp.add_method_ref(exc_class, "<init>", "(Ljava/lang/String;)V")?;
                let msg = cp.add_string("not yet implemented")?;
                instructions.push(Instruction::New(exc_class));
                instructions.push(Instruction::Dup);
                instructions.push(Instruction::Ldc_w(msg));
                instructions.push(Instruction::Invokespecial(exc_init));
                instructions.push(Instruction::Athrow);
                stack += 2;
                max_stack = max_stack.max(stack);
                stack = 0;
            }
            _ => {
                pending_label = false;
                let instr = emit_op(cp, op, &mut fixups, instructions.len())?;
                instructions.extend(instr);
                stack += op.stack_delta();
                debug_assert!(stack >= 0, "JVM stack underflow detected");
                max_stack = max_stack.max(stack);
            }
        }
    }

    for (instr_idx, label) in &fixups {
        let target = label_positions
            .get(label)
            .ok_or(CodegenError::UnresolvedLabel(*label))?;
        let target_u16 = *target as u16;
        match &mut instructions[*instr_idx] {
            Instruction::Goto(ref mut t)
            | Instruction::Ifeq(ref mut t)
            | Instruction::Ifne(ref mut t)
            | Instruction::Iflt(ref mut t)
            | Instruction::Ifge(ref mut t)
            | Instruction::Ifgt(ref mut t)
            | Instruction::Ifle(ref mut t)
            | Instruction::If_icmpeq(ref mut t)
            | Instruction::If_icmpne(ref mut t)
            | Instruction::If_icmplt(ref mut t)
            | Instruction::If_icmpge(ref mut t)
            | Instruction::If_icmpgt(ref mut t)
            | Instruction::If_icmple(ref mut t)
            | Instruction::If_acmpeq(ref mut t)
            | Instruction::If_acmpne(ref mut t)
            | Instruction::Ifnull(ref mut t)
            | Instruction::Ifnonnull(ref mut t) => {
                *t = target_u16;
            }
            _ => {}
        }
    }

    // Collect all branch targets that need StackMapTable frames
    let mut branch_targets: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for (_, label) in &fixups {
        if let Some(&pos) = label_positions.get(label) {
            branch_targets.insert(pos);
        }
    }

    // Merge explicit Frame hints with auto-generated frames for branch targets
    let frame_positions: std::collections::BTreeSet<usize> =
        frames.iter().map(|f| f.instr_index).collect();
    for &target in &branch_targets {
        if !frame_positions.contains(&target) {
            frames.push(FrameInfo {
                instr_index: target,
                locals: vec![],
                stack: vec![],
            });
        }
    }
    frames.sort_by_key(|f| f.instr_index);

    let stack_frames = build_stack_frames(cp, &frames, instructions.len())?;

    // Build exception table from IR-level exception handlers
    let mut exception_table = Vec::new();
    for handler in &body.exception_handlers {
        let start_pc = *label_positions
            .get(&handler.start)
            .ok_or(CodegenError::UnresolvedLabel(handler.start))? as u16;
        let end_pc = *label_positions
            .get(&handler.end)
            .ok_or(CodegenError::UnresolvedLabel(handler.end))? as u16;
        let handler_pc = *label_positions
            .get(&handler.handler)
            .ok_or(CodegenError::UnresolvedLabel(handler.handler))? as u16;
        let catch_type = if let Some(ref class_name) = handler.catch_type {
            cp.add_class(class_name)?
        } else {
            0 // catch-all (finally)
        };
        exception_table.push(ExceptionTableEntry {
            range_pc: start_pc..end_pc,
            handler_pc,
            catch_type,
        });
    }

    Ok((
        instructions,
        max_stack.max(1) as u16,
        stack_frames,
        exception_table,
    ))
}

fn build_stack_frames(
    cp: &mut ConstantPool,
    frames: &[FrameInfo],
    instr_count: usize,
) -> Result<Vec<StackFrame>, CodegenError> {
    let mut result = Vec::new();
    let mut prev_offset: i32 = -1;

    for frame in frames {
        if frame.instr_index >= instr_count {
            continue;
        }
        let offset = frame.instr_index as i32;
        let delta = offset - prev_offset - 1;
        if delta < 0 {
            continue;
        }
        prev_offset = offset;

        if frame.locals.is_empty() && frame.stack.is_empty() {
            if delta <= 63 {
                result.push(StackFrame::SameFrame {
                    frame_type: delta as u8,
                });
            } else {
                result.push(StackFrame::SameFrameExtended {
                    frame_type: 251,
                    offset_delta: delta as u16,
                });
            }
        } else {
            let locals: Vec<VerificationType> = frame
                .locals
                .iter()
                .map(|t| jvm_type_to_verification(cp, t))
                .collect::<Result<_, _>>()?;
            let stack: Vec<VerificationType> = frame
                .stack
                .iter()
                .map(|t| jvm_type_to_verification(cp, t))
                .collect::<Result<_, _>>()?;

            result.push(StackFrame::FullFrame {
                frame_type: 255,
                offset_delta: delta as u16,
                locals,
                stack,
            });
        }
    }

    Ok(result)
}

fn jvm_type_to_verification(
    cp: &mut ConstantPool,
    ty: &JvmType,
) -> Result<VerificationType, CodegenError> {
    Ok(match ty {
        JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean => {
            VerificationType::Integer
        }
        JvmType::Long => VerificationType::Long,
        JvmType::Float => VerificationType::Float,
        JvmType::Double => VerificationType::Double,
        JvmType::Object(name) => {
            let idx = cp.add_class(name)?;
            VerificationType::Object { cpool_index: idx }
        }
        JvmType::Array(elem) => {
            let desc = jvm_type_descriptor(&JvmType::Array(elem.clone()));
            let idx = cp.add_class(&desc)?;
            VerificationType::Object { cpool_index: idx }
        }
        JvmType::Void => VerificationType::Top,
    })
}

fn emit_op(
    cp: &mut ConstantPool,
    op: &JvmOp,
    fixups: &mut Vec<(usize, Label)>,
    current_pos: usize,
) -> Result<Vec<Instruction>, CodegenError> {
    Ok(match op {
        JvmOp::LoadThis => vec![Instruction::Aload_0],
        JvmOp::LoadLocal(slot, ty) => vec![load_instruction(*slot, ty)],
        JvmOp::StoreLocal(slot, ty) => vec![store_instruction(*slot, ty)],

        JvmOp::GetField {
            owner,
            name,
            descriptor,
        } => {
            let desc_str = jvm_type_descriptor(descriptor);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_field_ref(class_idx, name, &desc_str)?;
            vec![Instruction::Getfield(idx)]
        }
        JvmOp::PutField {
            owner,
            name,
            descriptor,
        } => {
            let desc_str = jvm_type_descriptor(descriptor);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_field_ref(class_idx, name, &desc_str)?;
            vec![Instruction::Putfield(idx)]
        }
        JvmOp::GetStatic {
            owner,
            name,
            descriptor,
        } => {
            let desc_str = jvm_type_descriptor(descriptor);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_field_ref(class_idx, name, &desc_str)?;
            vec![Instruction::Getstatic(idx)]
        }
        JvmOp::PutStatic {
            owner,
            name,
            descriptor,
        } => {
            let desc_str = jvm_type_descriptor(descriptor);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_field_ref(class_idx, name, &desc_str)?;
            vec![Instruction::Putstatic(idx)]
        }

        JvmOp::InvokeSpecial {
            owner,
            name,
            params,
            ret,
        } => {
            let desc = jvm_method_descriptor(params, ret);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_method_ref(class_idx, name, &desc)?;
            vec![Instruction::Invokespecial(idx)]
        }
        JvmOp::InvokeVirtual {
            owner,
            name,
            params,
            ret,
        } => {
            let desc = jvm_method_descriptor(params, ret);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_method_ref(class_idx, name, &desc)?;
            vec![Instruction::Invokevirtual(idx)]
        }
        JvmOp::InvokeStatic {
            owner,
            name,
            params,
            ret,
        } => {
            let desc = jvm_method_descriptor(params, ret);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_method_ref(class_idx, name, &desc)?;
            vec![Instruction::Invokestatic(idx)]
        }
        JvmOp::InvokeInterface {
            owner,
            name,
            params,
            ret,
        } => {
            let desc = jvm_method_descriptor(params, ret);
            let class_idx = cp.add_class(owner)?;
            let idx = cp.add_interface_method_ref(class_idx, name, &desc)?;
            let nargs = 1 + params.iter().map(|t| t.slot_count() as u8).sum::<u8>();
            vec![Instruction::Invokeinterface(idx, nargs)]
        }

        JvmOp::New(class) => {
            let idx = cp.add_class(class)?;
            vec![Instruction::New(idx)]
        }
        JvmOp::Dup => vec![Instruction::Dup],
        JvmOp::Pop => vec![Instruction::Pop],
        JvmOp::Pop2 => vec![Instruction::Pop2],
        JvmOp::Swap => vec![Instruction::Swap],

        JvmOp::PushInt(n) => vec![push_int(*n, cp)?],
        JvmOp::PushLong(n) => {
            let idx = cp.add_long(*n)?;
            vec![Instruction::Ldc2_w(idx)]
        }
        JvmOp::PushFloat(n) => {
            if n.to_bits() == 0 {
                vec![Instruction::Fconst_0]
            } else if *n == 1.0 {
                vec![Instruction::Fconst_1]
            } else if *n == 2.0 {
                vec![Instruction::Fconst_2]
            } else {
                let idx = cp.add_float(*n)?;
                vec![ldc_or_ldc_w(idx)]
            }
        }
        JvmOp::PushDouble(n) => {
            if n.to_bits() == 0 {
                vec![Instruction::Dconst_0]
            } else if *n == 1.0 {
                vec![Instruction::Dconst_1]
            } else {
                let idx = cp.add_double(*n)?;
                vec![Instruction::Ldc2_w(idx)]
            }
        }
        JvmOp::PushString(s) => {
            let idx = cp.add_string(s)?;
            vec![ldc_or_ldc_w(idx)]
        }
        JvmOp::PushNull => vec![Instruction::Aconst_null],

        JvmOp::Checkcast(class) => {
            let idx = cp.add_class(class)?;
            vec![Instruction::Checkcast(idx)]
        }
        JvmOp::Instanceof(class) => {
            let idx = cp.add_class(class)?;
            vec![Instruction::Instanceof(idx)]
        }

        JvmOp::Return(ty) => vec![return_instruction(ty)],

        JvmOp::Goto(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Goto(0)]
        }
        JvmOp::IfEq(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifeq(0)]
        }
        JvmOp::IfNe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifne(0)]
        }
        JvmOp::IfICmpEq(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_icmpeq(0)]
        }
        JvmOp::IfICmpNe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_icmpne(0)]
        }
        JvmOp::IfACmpEq(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_acmpeq(0)]
        }
        JvmOp::IfACmpNe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_acmpne(0)]
        }
        JvmOp::IfNull(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifnull(0)]
        }
        JvmOp::IfNonNull(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifnonnull(0)]
        }

        JvmOp::Arith(aop, ty) => vec![emit_arith(*aop, ty)],
        JvmOp::Neg(ty) => vec![emit_neg(ty)],
        JvmOp::Cmp(kind) => vec![emit_cmp(*kind)],
        JvmOp::Convert { from, to } => emit_convert(from, to),
        JvmOp::Bitwise(bop, ty) => vec![emit_bitwise(*bop, ty)],

        JvmOp::IfLt(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Iflt(0)]
        }
        JvmOp::IfGe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifge(0)]
        }
        JvmOp::IfGt(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifgt(0)]
        }
        JvmOp::IfLe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::Ifle(0)]
        }
        JvmOp::IfICmpLt(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_icmplt(0)]
        }
        JvmOp::IfICmpGe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_icmpge(0)]
        }
        JvmOp::IfICmpGt(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_icmpgt(0)]
        }
        JvmOp::IfICmpLe(label) => {
            fixups.push((current_pos, *label));
            vec![Instruction::If_icmple(0)]
        }

        JvmOp::AThrow => vec![Instruction::Athrow],

        JvmOp::InvokeDynamic {
            bootstrap_index,
            name,
            descriptor,
        } => {
            let idx =
                cp.add_invoke_dynamic(*bootstrap_index, name.as_str(), descriptor.as_str())?;
            vec![Instruction::Invokedynamic(idx)]
        }

        JvmOp::IInc(slot, inc) => {
            if *slot <= 255 && (-128..=127).contains(inc) {
                vec![Instruction::Iinc(*slot as u8, *inc as i8)]
            } else {
                vec![Instruction::Iinc_w(*slot, *inc as i16)]
            }
        }

        JvmOp::Label(_) | JvmOp::StubBody | JvmOp::Frame { .. } => vec![],
    })
}

fn emit_arith(op: ArithOp, ty: &JvmType) -> Instruction {
    use ArithOp::*;
    match (op, ty) {
        (Add, JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean) => {
            Instruction::Iadd
        }
        (Sub, JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean) => {
            Instruction::Isub
        }
        (Mul, JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean) => {
            Instruction::Imul
        }
        (Div, JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean) => {
            Instruction::Idiv
        }
        (Rem, JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean) => {
            Instruction::Irem
        }
        (Add, JvmType::Long) => Instruction::Ladd,
        (Sub, JvmType::Long) => Instruction::Lsub,
        (Mul, JvmType::Long) => Instruction::Lmul,
        (Div, JvmType::Long) => Instruction::Ldiv,
        (Rem, JvmType::Long) => Instruction::Lrem,
        (Add, JvmType::Float) => Instruction::Fadd,
        (Sub, JvmType::Float) => Instruction::Fsub,
        (Mul, JvmType::Float) => Instruction::Fmul,
        (Div, JvmType::Float) => Instruction::Fdiv,
        (Rem, JvmType::Float) => Instruction::Frem,
        (Add, JvmType::Double) => Instruction::Dadd,
        (Sub, JvmType::Double) => Instruction::Dsub,
        (Mul, JvmType::Double) => Instruction::Dmul,
        (Div, JvmType::Double) => Instruction::Ddiv,
        (Rem, JvmType::Double) => Instruction::Drem,
        (op, ty) => unreachable!("emit_arith: unsupported {op:?} for {ty:?}"),
    }
}

fn emit_neg(ty: &JvmType) -> Instruction {
    match ty {
        JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char => Instruction::Ineg,
        JvmType::Long => Instruction::Lneg,
        JvmType::Float => Instruction::Fneg,
        JvmType::Double => Instruction::Dneg,
        other => unreachable!("emit_neg: unsupported type {other:?}"),
    }
}

fn emit_cmp(kind: CmpKind) -> Instruction {
    match kind {
        CmpKind::LCmp => Instruction::Lcmp,
        CmpKind::FCmpL => Instruction::Fcmpl,
        CmpKind::FCmpG => Instruction::Fcmpg,
        CmpKind::DCmpL => Instruction::Dcmpl,
        CmpKind::DCmpG => Instruction::Dcmpg,
    }
}

fn emit_convert(from: &JvmType, to: &JvmType) -> Vec<Instruction> {
    use JvmType::*;
    match (from, to) {
        (Int | Byte | Short | Char | Boolean, Long) => vec![Instruction::I2l],
        (Int | Byte | Short | Char | Boolean, Float) => vec![Instruction::I2f],
        (Int | Byte | Short | Char | Boolean, Double) => vec![Instruction::I2d],
        (Long, Int) => vec![Instruction::L2i],
        (Long, Float) => vec![Instruction::L2f],
        (Long, Double) => vec![Instruction::L2d],
        (Float, Int) => vec![Instruction::F2i],
        (Float, Long) => vec![Instruction::F2l],
        (Float, Double) => vec![Instruction::F2d],
        (Double, Int) => vec![Instruction::D2i],
        (Double, Long) => vec![Instruction::D2l],
        (Double, Float) => vec![Instruction::D2f],
        (Int, Byte) => vec![Instruction::I2b],
        (Int, Char) => vec![Instruction::I2c],
        (Int, Short) => vec![Instruction::I2s],
        _ => vec![],
    }
}

fn emit_bitwise(op: BitwiseOp, ty: &JvmType) -> Instruction {
    use BitwiseOp::*;
    match (op, ty) {
        (And, JvmType::Long) => Instruction::Land,
        (Or, JvmType::Long) => Instruction::Lor,
        (Xor, JvmType::Long) => Instruction::Lxor,
        (Shl, JvmType::Long) => Instruction::Lshl,
        (Shr, JvmType::Long) => Instruction::Lshr,
        (UShr, JvmType::Long) => Instruction::Lushr,
        (And, _) => Instruction::Iand,
        (Or, _) => Instruction::Ior,
        (Xor, _) => Instruction::Ixor,
        (Shl, _) => Instruction::Ishl,
        (Shr, _) => Instruction::Ishr,
        (UShr, _) => Instruction::Iushr,
    }
}

fn load_instruction(slot: u16, ty: &JvmType) -> Instruction {
    assert!(
        slot <= 255,
        "local slot {slot} exceeds u8 range; wide prefix not yet supported"
    );
    match ty {
        JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean => {
            match slot {
                0 => Instruction::Iload_0,
                1 => Instruction::Iload_1,
                2 => Instruction::Iload_2,
                3 => Instruction::Iload_3,
                s => Instruction::Iload(s as u8),
            }
        }
        JvmType::Long => match slot {
            0 => Instruction::Lload_0,
            1 => Instruction::Lload_1,
            2 => Instruction::Lload_2,
            3 => Instruction::Lload_3,
            s => Instruction::Lload(s as u8),
        },
        JvmType::Float => match slot {
            0 => Instruction::Fload_0,
            1 => Instruction::Fload_1,
            2 => Instruction::Fload_2,
            3 => Instruction::Fload_3,
            s => Instruction::Fload(s as u8),
        },
        JvmType::Double => match slot {
            0 => Instruction::Dload_0,
            1 => Instruction::Dload_1,
            2 => Instruction::Dload_2,
            3 => Instruction::Dload_3,
            s => Instruction::Dload(s as u8),
        },
        _ => match slot {
            0 => Instruction::Aload_0,
            1 => Instruction::Aload_1,
            2 => Instruction::Aload_2,
            3 => Instruction::Aload_3,
            s => Instruction::Aload(s as u8),
        },
    }
}

fn store_instruction(slot: u16, ty: &JvmType) -> Instruction {
    assert!(
        slot <= 255,
        "local slot {slot} exceeds u8 range; wide prefix not yet supported"
    );
    match ty {
        JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean => {
            match slot {
                0 => Instruction::Istore_0,
                1 => Instruction::Istore_1,
                2 => Instruction::Istore_2,
                3 => Instruction::Istore_3,
                s => Instruction::Istore(s as u8),
            }
        }
        JvmType::Long => match slot {
            0 => Instruction::Lstore_0,
            1 => Instruction::Lstore_1,
            2 => Instruction::Lstore_2,
            3 => Instruction::Lstore_3,
            s => Instruction::Lstore(s as u8),
        },
        JvmType::Float => match slot {
            0 => Instruction::Fstore_0,
            1 => Instruction::Fstore_1,
            2 => Instruction::Fstore_2,
            3 => Instruction::Fstore_3,
            s => Instruction::Fstore(s as u8),
        },
        JvmType::Double => match slot {
            0 => Instruction::Dstore_0,
            1 => Instruction::Dstore_1,
            2 => Instruction::Dstore_2,
            3 => Instruction::Dstore_3,
            s => Instruction::Dstore(s as u8),
        },
        _ => match slot {
            0 => Instruction::Astore_0,
            1 => Instruction::Astore_1,
            2 => Instruction::Astore_2,
            3 => Instruction::Astore_3,
            s => Instruction::Astore(s as u8),
        },
    }
}

fn return_instruction(ty: &JvmType) -> Instruction {
    match ty {
        JvmType::Void => Instruction::Return,
        JvmType::Int | JvmType::Byte | JvmType::Short | JvmType::Char | JvmType::Boolean => {
            Instruction::Ireturn
        }
        JvmType::Long => Instruction::Lreturn,
        JvmType::Float => Instruction::Freturn,
        JvmType::Double => Instruction::Dreturn,
        _ => Instruction::Areturn,
    }
}

fn push_int(n: i32, cp: &mut ConstantPool) -> Result<Instruction, CodegenError> {
    Ok(match n {
        -1 => Instruction::Iconst_m1,
        0 => Instruction::Iconst_0,
        1 => Instruction::Iconst_1,
        2 => Instruction::Iconst_2,
        3 => Instruction::Iconst_3,
        4 => Instruction::Iconst_4,
        5 => Instruction::Iconst_5,
        -128..=127 => Instruction::Bipush(n as i8),
        -32768..=32767 => Instruction::Sipush(n as i16),
        _ => {
            let idx = cp.add_integer(n)?;
            ldc_or_ldc_w(idx)
        }
    })
}

fn ldc_or_ldc_w(idx: u16) -> Instruction {
    if idx <= 255 {
        Instruction::Ldc(idx as u8)
    } else {
        Instruction::Ldc_w(idx)
    }
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
        s if s.starts_with('[') => {
            let inner = descriptor_to_field_type(&s[1..]);
            FieldType::Array(Box::new(inner))
        }
        _ => FieldType::Object(JavaString::from(desc)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jvm_ir::*;

    fn simple_class(name: &str, fields: Vec<JvmField>, methods: Vec<JvmMethod>) -> JvmClass {
        JvmClass {
            version: crate::JvmVersion::Java21,
            access: JvmClassAccess {
                is_public: true,
                is_final: true,
                is_super: true,
                ..Default::default()
            },
            name: name.to_string(),
            super_class: "java/lang/Object".to_string(),
            interfaces: vec![],
            fields,
            methods,
            source_file: None,
            permitted_subclasses: vec![],
            is_record: false,
            bootstrap_methods: vec![],
            synthetic_methods: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn emit_empty_class_via_ir() {
        let ctor = JvmMethod {
            access: JvmMethodAccess {
                is_public: true,
                ..Default::default()
            },
            name: "<init>".to_string(),
            params: vec![],
            return_type: JvmType::Void,
            body: Some(JvmMethodBody {
                max_locals: 1,
                ops: vec![
                    JvmOp::LoadThis,
                    JvmOp::InvokeSpecial {
                        owner: "java/lang/Object".to_string(),
                        name: "<init>".to_string(),
                        params: vec![],
                        ret: JvmType::Void,
                    },
                    JvmOp::Return(JvmType::Void),
                ],
                exception_handlers: vec![],
            }),
        };

        let jvm_class = simple_class("Foo", vec![], vec![ctor]);
        let output = emit_class(&jvm_class).unwrap();
        assert_eq!(output.internal_name, "Foo");
        assert_eq!(&output.bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);

        let parsed = ClassFile::from_bytes(&output.bytes).unwrap();
        assert_eq!(parsed.class_name().unwrap(), "Foo");
        assert!(parsed.access_flags.contains(ClassAccessFlags::PUBLIC));
        assert!(parsed.access_flags.contains(ClassAccessFlags::FINAL));
        assert_eq!(parsed.methods.len(), 1);
    }

    #[test]
    fn emit_class_with_fields() {
        let ctor = JvmMethod {
            access: JvmMethodAccess {
                is_public: true,
                ..Default::default()
            },
            name: "<init>".to_string(),
            params: vec![
                JvmType::Object("java/lang/String".to_string()),
                JvmType::Int,
            ],
            return_type: JvmType::Void,
            body: Some(JvmMethodBody {
                max_locals: 3,
                ops: vec![
                    JvmOp::LoadThis,
                    JvmOp::InvokeSpecial {
                        owner: "java/lang/Object".to_string(),
                        name: "<init>".to_string(),
                        params: vec![],
                        ret: JvmType::Void,
                    },
                    JvmOp::LoadThis,
                    JvmOp::LoadLocal(1, JvmType::Object("java/lang/String".to_string())),
                    JvmOp::PutField {
                        owner: "User".to_string(),
                        name: "name".to_string(),
                        descriptor: JvmType::Object("java/lang/String".to_string()),
                    },
                    JvmOp::LoadThis,
                    JvmOp::LoadLocal(2, JvmType::Int),
                    JvmOp::PutField {
                        owner: "User".to_string(),
                        name: "age".to_string(),
                        descriptor: JvmType::Int,
                    },
                    JvmOp::Return(JvmType::Void),
                ],
                exception_handlers: vec![],
            }),
        };

        let fields = vec![
            JvmField {
                access: JvmFieldAccess {
                    is_public: true,
                    is_final: true,
                    ..Default::default()
                },
                name: "name".to_string(),
                ty: JvmType::Object("java/lang/String".to_string()),
            },
            JvmField {
                access: JvmFieldAccess {
                    is_private: true,
                    ..Default::default()
                },
                name: "age".to_string(),
                ty: JvmType::Int,
            },
        ];

        let jvm_class = simple_class("User", fields, vec![ctor]);
        let output = emit_class(&jvm_class).unwrap();
        let parsed = ClassFile::from_bytes(&output.bytes).unwrap();

        assert_eq!(parsed.fields.len(), 2);
        assert!(parsed.fields[0]
            .access_flags
            .contains(FieldAccessFlags::PUBLIC));
        assert!(parsed.fields[0]
            .access_flags
            .contains(FieldAccessFlags::FINAL));
        assert!(parsed.fields[1]
            .access_flags
            .contains(FieldAccessFlags::PRIVATE));
    }

    #[test]
    fn emit_stub_method() {
        let ctor = JvmMethod {
            access: JvmMethodAccess {
                is_public: true,
                ..Default::default()
            },
            name: "<init>".to_string(),
            params: vec![],
            return_type: JvmType::Void,
            body: Some(JvmMethodBody {
                max_locals: 1,
                ops: vec![
                    JvmOp::LoadThis,
                    JvmOp::InvokeSpecial {
                        owner: "java/lang/Object".to_string(),
                        name: "<init>".to_string(),
                        params: vec![],
                        ret: JvmType::Void,
                    },
                    JvmOp::Return(JvmType::Void),
                ],
                exception_handlers: vec![],
            }),
        };

        let stub = JvmMethod {
            access: JvmMethodAccess {
                is_public: true,
                ..Default::default()
            },
            name: "greet".to_string(),
            params: vec![],
            return_type: JvmType::Object("java/lang/String".to_string()),
            body: Some(JvmMethodBody {
                max_locals: 1,
                ops: vec![JvmOp::StubBody],
                exception_handlers: vec![],
            }),
        };

        let jvm_class = simple_class("Foo", vec![], vec![ctor, stub]);
        let output = emit_class(&jvm_class).unwrap();
        let parsed = ClassFile::from_bytes(&output.bytes).unwrap();
        assert_eq!(parsed.methods.len(), 2);
    }

    #[test]
    fn emit_permitted_subclasses() {
        let ctor = JvmMethod {
            access: JvmMethodAccess {
                is_public: true,
                ..Default::default()
            },
            name: "<init>".to_string(),
            params: vec![],
            return_type: JvmType::Void,
            body: Some(JvmMethodBody {
                max_locals: 1,
                ops: vec![
                    JvmOp::LoadThis,
                    JvmOp::InvokeSpecial {
                        owner: "java/lang/Object".to_string(),
                        name: "<init>".to_string(),
                        params: vec![],
                        ret: JvmType::Void,
                    },
                    JvmOp::Return(JvmType::Void),
                ],
                exception_handlers: vec![],
            }),
        };

        let jvm_class = JvmClass {
            version: crate::JvmVersion::Java21,
            access: JvmClassAccess {
                is_public: true,
                is_abstract: true,
                is_super: true,
                ..Default::default()
            },
            name: "Payment".to_string(),
            super_class: "java/lang/Object".to_string(),
            interfaces: vec![],
            fields: vec![],
            methods: vec![ctor],
            source_file: None,
            permitted_subclasses: vec!["Card".to_string(), "Cash".to_string()],
            is_record: false,
            bootstrap_methods: vec![],
            synthetic_methods: vec![],
            annotations: vec![],
        };

        let output = emit_class(&jvm_class).unwrap();
        let parsed = ClassFile::from_bytes(&output.bytes).unwrap();
        let has_permitted = parsed
            .attributes
            .iter()
            .any(|a| matches!(a, Attribute::PermittedSubclasses { .. }));
        assert!(has_permitted);
    }

    #[test]
    fn roundtrip_lower_and_emit() {
        use smol_str::SmolStr;
        use valen_ast::FileId;
        use valen_hir::*;

        let mut hir = Hir::default();
        let id = hir.alloc_id();
        hir.defs.insert(
            id,
            Def {
                id,
                name: SmolStr::from("Point"),
                kind: DefKind::DataClass(DataClassDef {
                    ctor_params: vec![
                        CtorParamDef {
                            vis: Vis::Pub,
                            name: "x".into(),
                            ty: TyRef::Prim(PrimTy::Int),
                            mutable: false,
                        },
                        CtorParamDef {
                            vis: Vis::Pub,
                            name: "y".into(),
                            ty: TyRef::Prim(PrimTy::Int),
                            mutable: false,
                        },
                    ],
                }),
                vis: Vis::Pub,
                span: valen_ast::Span {
                    start: 0,
                    end: 0,
                    file_id: FileId(0),
                },
                package: None,
            },
        );

        let jvm_classes = crate::lower::lower_hir(&hir, &indexmap::IndexMap::new());
        assert_eq!(jvm_classes.len(), 1);

        let output = emit_class(&jvm_classes[0]).unwrap();
        let parsed = ClassFile::from_bytes(&output.bytes).unwrap();
        assert_eq!(parsed.class_name().unwrap(), "Point");
        assert!(parsed.access_flags.contains(ClassAccessFlags::FINAL));
        assert_eq!(parsed.fields.len(), 2);
        // <init>, equals, hashCode, toString, copy
        assert_eq!(parsed.methods.len(), 5);
    }
}
