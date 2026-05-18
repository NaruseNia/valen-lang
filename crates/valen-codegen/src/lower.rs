//! Lowers `valen_hir::Hir` definitions into `JvmClass` IR.

use indexmap::IndexMap;
use smol_str::SmolStr;
use valen_hir::{ClassDefKind, Def, DefId, DefKind, FnDef, Hir, TypedBody, Vis};

use crate::data_class_methods;
use crate::descriptor::{class_internal_name, tyref_to_jvm};
use crate::jvm_const::*;
use crate::jvm_ir::{
    JvmBootstrapMethod, JvmClass, JvmClassAccess, JvmField, JvmFieldAccess, JvmMethod,
    JvmMethodAccess, JvmMethodBody, JvmOp, JvmType, SyntheticLambda,
};
use crate::JvmVersion;

/// Lowers an entire HIR module into a list of JVM class IR nodes.
pub fn lower_hir(hir: &Hir, typed_bodies: &IndexMap<DefId, TypedBody>) -> Vec<JvmClass> {
    let mut classes = Vec::new();

    for (id, def) in &hir.defs {
        if hir.prelude_ids.contains(id) {
            continue;
        }
        let pkg = def.package.as_deref().or(hir.package.as_deref());
        let source_file = Some(format!("{}.vln", def.name));
        match &def.kind {
            DefKind::Class(class_def) => {
                classes.push(lower_class(
                    hir,
                    def,
                    class_def,
                    typed_bodies,
                    pkg,
                    source_file,
                ));
            }
            DefKind::DataClass(data_def) => {
                classes.push(lower_data_class(
                    hir,
                    def,
                    data_def,
                    typed_bodies,
                    pkg,
                    source_file,
                ));
            }
            DefKind::Enum(enum_def) => {
                classes.extend(lower_enum(
                    hir,
                    def,
                    enum_def,
                    typed_bodies,
                    pkg,
                    source_file,
                ));
            }
            DefKind::Trait(trait_def) if trait_def.is_sealed => {
                classes.push(lower_sealed_trait(hir, def, pkg, source_file));
            }
            DefKind::AnnotationClass(ann_def) => {
                classes.push(lower_annotation_class(def, ann_def, pkg, source_file));
            }
            _ => {}
        }
    }

    classes.push(generate_list_iterator_class());

    classes
}

/// Generates a synthetic `valen/core/ListIterator` class that wraps a `java.util.List`
/// and implements `valen/core/Iterator` by sequentially returning elements via `next()`.
fn generate_list_iterator_class() -> JvmClass {
    use crate::jvm_ir::*;
    let class_name = "valen/core/ListIterator";
    let obj = JvmType::Object(JVM_OBJECT.to_string());

    // <init>(Ljava/util/List;)V
    let ctor = JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: "<init>".to_string(),
        params: vec![JvmType::Object("java/util/List".to_string())],
        return_type: JvmType::Void,
        body: Some(JvmMethodBody {
            max_locals: 3,
            ops: vec![
                // super.<init>()
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::InvokeSpecial {
                    owner: JVM_OBJECT.to_string(),
                    name: "<init>".to_string(),
                    params: vec![],
                    ret: JvmType::Void,
                },
                // this.list = arg
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::LoadLocal(1, JvmType::Object("java/util/List".to_string())),
                JvmOp::PutField {
                    owner: class_name.to_string(),
                    name: "list".to_string(),
                    descriptor: JvmType::Object("java/util/List".to_string()),
                },
                // this.index = 0
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::PushInt(0),
                JvmOp::PutField {
                    owner: class_name.to_string(),
                    name: "index".to_string(),
                    descriptor: JvmType::Int,
                },
                JvmOp::Return(JvmType::Void),
            ],
            exception_handlers: vec![],
        }),
    };

    // next() -> Option<T>  (returns Option$Some or Option$None)
    let label_none: Label = 0;
    let _label_end: Label = 1;
    let next_method = JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: "next".to_string(),
        params: vec![],
        return_type: JvmType::Object("valen/core/Option".to_string()),
        body: Some(JvmMethodBody {
            max_locals: 3,
            ops: vec![
                // if (this.index >= this.list.size()) goto none
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::GetField {
                    owner: class_name.to_string(),
                    name: "index".to_string(),
                    descriptor: JvmType::Int,
                },
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::GetField {
                    owner: class_name.to_string(),
                    name: "list".to_string(),
                    descriptor: JvmType::Object("java/util/List".to_string()),
                },
                JvmOp::InvokeInterface {
                    owner: "java/util/List".to_string(),
                    name: "size".to_string(),
                    params: vec![],
                    ret: JvmType::Int,
                },
                JvmOp::IfICmpGe(label_none),
                // Object elem = this.list.get(this.index)
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::GetField {
                    owner: class_name.to_string(),
                    name: "list".to_string(),
                    descriptor: JvmType::Object("java/util/List".to_string()),
                },
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::GetField {
                    owner: class_name.to_string(),
                    name: "index".to_string(),
                    descriptor: JvmType::Int,
                },
                JvmOp::InvokeInterface {
                    owner: "java/util/List".to_string(),
                    name: "get".to_string(),
                    params: vec![JvmType::Int],
                    ret: obj.clone(),
                },
                JvmOp::StoreLocal(1, obj.clone()),
                // this.index++
                JvmOp::LoadLocal(0, obj.clone()),
                JvmOp::Dup,
                JvmOp::GetField {
                    owner: class_name.to_string(),
                    name: "index".to_string(),
                    descriptor: JvmType::Int,
                },
                JvmOp::PushInt(1),
                JvmOp::Arith(ArithOp::Add, JvmType::Int),
                JvmOp::PutField {
                    owner: class_name.to_string(),
                    name: "index".to_string(),
                    descriptor: JvmType::Int,
                },
                // return new Some(elem)
                JvmOp::New("valen/core/Option$Some".to_string()),
                JvmOp::Dup,
                JvmOp::LoadLocal(1, obj.clone()),
                JvmOp::InvokeSpecial {
                    owner: "valen/core/Option$Some".to_string(),
                    name: "<init>".to_string(),
                    params: vec![obj.clone()],
                    ret: JvmType::Void,
                },
                JvmOp::Return(JvmType::Object("valen/core/Option".to_string())),
                // none:
                JvmOp::Label(label_none),
                JvmOp::Frame {
                    locals: vec![JvmType::Object(class_name.to_string())],
                    stack: vec![],
                },
                JvmOp::New("valen/core/Option$None".to_string()),
                JvmOp::Dup,
                JvmOp::InvokeSpecial {
                    owner: "valen/core/Option$None".to_string(),
                    name: "<init>".to_string(),
                    params: vec![],
                    ret: JvmType::Void,
                },
                JvmOp::Return(JvmType::Object("valen/core/Option".to_string())),
            ],
            exception_handlers: vec![],
        }),
    };

    JvmClass {
        version: crate::JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: true,
            is_final: true,
            ..Default::default()
        },
        name: class_name.to_string(),
        super_class: JVM_OBJECT.to_string(),
        interfaces: vec!["valen/core/Iterator".to_string()],
        fields: vec![
            JvmField {
                access: JvmFieldAccess {
                    is_private: true,
                    is_final: true,
                    ..Default::default()
                },
                name: "list".to_string(),
                ty: JvmType::Object("java/util/List".to_string()),
            },
            JvmField {
                access: JvmFieldAccess {
                    is_private: true,
                    ..Default::default()
                },
                name: "index".to_string(),
                ty: JvmType::Int,
            },
        ],
        methods: vec![ctor, next_method],
        source_file: Some("ListIterator.vln".to_string()),
        permitted_subclasses: vec![],
        is_record: false,
        bootstrap_methods: vec![],
        synthetic_methods: vec![],
        annotations: vec![],
    }
}

fn lower_class(
    hir: &Hir,
    def: &Def,
    class_def: &valen_hir::ClassDef,
    typed_bodies: &IndexMap<DefId, TypedBody>,
    pkg: Option<&[SmolStr]>,
    source_file: Option<String>,
) -> JvmClass {
    let internal = class_internal_name(&def.name, pkg);

    let super_class = class_def
        .superclass
        .as_ref()
        .map(|s| match &tyref_to_jvm(s, pkg, &hir.imports) {
            JvmType::Object(name) => name.clone(),
            _ => JVM_OBJECT.to_string(),
        })
        .unwrap_or_else(|| JVM_OBJECT.to_string());

    let mut interfaces: Vec<String> = class_def
        .trait_impls
        .iter()
        .filter_map(|t| match &tyref_to_jvm(t, pkg, &hir.imports) {
            JvmType::Object(name) => Some(name.clone()),
            _ => None,
        })
        .collect();

    // Add sealed trait interfaces from impl blocks
    for entry in &hir.trait_impls {
        if entry.target_name == def.name && is_sealed_trait_def(hir, &entry.trait_name) {
            let iface = class_internal_name(&entry.trait_name, pkg);
            if !interfaces.contains(&iface) {
                interfaces.push(iface);
            }
        }
    }

    let fields: Vec<JvmField> = class_def
        .ctor_params
        .iter()
        .map(|p| lower_field(p, pkg, &hir.imports))
        .collect();

    let mut methods = Vec::new();
    let mut all_synthetic_lambdas = Vec::new();
    let mut all_bootstrap_methods = Vec::new();

    methods.push(generate_ctor(&internal, &super_class, &fields));

    for &mid in &class_def.methods {
        if let Some(method_def) = hir.defs.get(&mid) {
            if let DefKind::Fn(fn_def) = &method_def.kind {
                let body = typed_bodies.get(&mid);
                let result = lower_method(hir, method_def, fn_def, body, &internal, pkg);
                methods.push(result.method);
                all_synthetic_lambdas.extend(result.synthetic_lambdas);
                all_bootstrap_methods.extend(result.bootstrap_methods);
            }
        }
    }

    // Collect methods from trait impls targeting this class
    for impl_entry in &hir.trait_impls {
        if impl_entry.target_name == def.name {
            for &mid in &impl_entry.methods {
                if let Some(method_def) = hir.defs.get(&mid) {
                    if let DefKind::Fn(fn_def) = &method_def.kind {
                        let body = typed_bodies.get(&mid);
                        let result = lower_method(hir, method_def, fn_def, body, &internal, pkg);
                        methods.push(result.method);
                        all_synthetic_lambdas.extend(result.synthetic_lambdas);
                        all_bootstrap_methods.extend(result.bootstrap_methods);
                    }
                }
            }
        }
    }

    // Collect methods from inherent impls (registered in type_methods by resolver)
    if let Some(type_method_ids) = hir.type_methods.get(&def.name) {
        let class_method_set: std::collections::HashSet<_> =
            class_def.methods.iter().copied().collect();
        for &mid in type_method_ids {
            if class_method_set.contains(&mid) {
                continue;
            }
            if let Some(method_def) = hir.defs.get(&mid) {
                if let DefKind::Fn(fn_def) = &method_def.kind {
                    let body = typed_bodies.get(&mid);
                    let result = lower_method(hir, method_def, fn_def, body, &internal, pkg);
                    methods.push(result.method);
                    all_synthetic_lambdas.extend(result.synthetic_lambdas);
                    all_bootstrap_methods.extend(result.bootstrap_methods);
                }
            }
        }
    }

    let permitted = collect_permitted_subclasses(hir, &def.name, pkg);

    // Convert collected synthetic lambdas to JvmMethod entries.
    let synthetic_methods = synthetic_lambdas_to_methods(all_synthetic_lambdas);

    JvmClass {
        version: JvmVersion::Java21,
        access: class_access(&def.vis, &class_def.kind),
        name: internal,
        super_class,
        interfaces,
        fields,
        methods,
        source_file,
        permitted_subclasses: permitted,
        is_record: false,
        bootstrap_methods: all_bootstrap_methods,
        synthetic_methods,
        annotations: vec![],
    }
}

fn lower_sealed_trait(
    hir: &Hir,
    def: &Def,
    pkg: Option<&[SmolStr]>,
    source_file: Option<String>,
) -> JvmClass {
    let internal = class_internal_name(&def.name, pkg);

    let permitted: Vec<String> = hir
        .trait_impls
        .iter()
        .filter(|entry| entry.trait_name == def.name)
        .map(|entry| class_internal_name(&entry.target_name, pkg))
        .collect();

    JvmClass {
        version: JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: matches!(def.vis, Vis::Pub),
            is_abstract: true,
            is_interface: true,
            ..Default::default()
        },
        name: internal,
        super_class: JVM_OBJECT.to_string(),
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        source_file,
        permitted_subclasses: permitted,
        is_record: false,
        bootstrap_methods: vec![],
        synthetic_methods: vec![],
        annotations: vec![],
    }
}

fn lower_annotation_class(
    def: &Def,
    ann_def: &valen_hir::AnnotationClassDef,
    pkg: Option<&[SmolStr]>,
    source_file: Option<String>,
) -> JvmClass {
    let internal = class_internal_name(&def.name, pkg);

    let methods: Vec<JvmMethod> = ann_def
        .params
        .iter()
        .map(|p| {
            let return_type = tyref_to_jvm(&p.ty, pkg, &Default::default());
            JvmMethod {
                access: JvmMethodAccess {
                    is_public: true,
                    is_abstract: true,
                    ..Default::default()
                },
                name: p.name.to_string(),
                params: vec![],
                return_type,
                body: None,
            }
        })
        .collect();

    let mut annotations = Vec::new();

    // @Retention(RUNTIME)
    annotations.push(crate::jvm_ir::JvmAnnotation {
        type_descriptor: "Ljava/lang/annotation/Retention;".to_string(),
        values: vec![(
            "value".to_string(),
            crate::jvm_ir::JvmAnnotationValue::Enum {
                type_name: "Ljava/lang/annotation/RetentionPolicy;".to_string(),
                const_name: "RUNTIME".to_string(),
            },
        )],
    });

    // @Target(...)
    if !ann_def.targets.is_empty() {
        let target_values: Vec<crate::jvm_ir::JvmAnnotationValue> = ann_def
            .targets
            .iter()
            .map(|t| {
                let element_type = match t.as_str() {
                    "type" => "TYPE",
                    "field" => "FIELD",
                    "method" => "METHOD",
                    other => other,
                };
                crate::jvm_ir::JvmAnnotationValue::Enum {
                    type_name: "Ljava/lang/annotation/ElementType;".to_string(),
                    const_name: element_type.to_string(),
                }
            })
            .collect();
        annotations.push(crate::jvm_ir::JvmAnnotation {
            type_descriptor: "Ljava/lang/annotation/Target;".to_string(),
            values: vec![(
                "value".to_string(),
                crate::jvm_ir::JvmAnnotationValue::Array(target_values),
            )],
        });
    } else {
        // Default: TYPE + FIELD + METHOD
        annotations.push(crate::jvm_ir::JvmAnnotation {
            type_descriptor: "Ljava/lang/annotation/Target;".to_string(),
            values: vec![(
                "value".to_string(),
                crate::jvm_ir::JvmAnnotationValue::Array(vec![
                    crate::jvm_ir::JvmAnnotationValue::Enum {
                        type_name: "Ljava/lang/annotation/ElementType;".to_string(),
                        const_name: "TYPE".to_string(),
                    },
                    crate::jvm_ir::JvmAnnotationValue::Enum {
                        type_name: "Ljava/lang/annotation/ElementType;".to_string(),
                        const_name: "FIELD".to_string(),
                    },
                    crate::jvm_ir::JvmAnnotationValue::Enum {
                        type_name: "Ljava/lang/annotation/ElementType;".to_string(),
                        const_name: "METHOD".to_string(),
                    },
                ]),
            )],
        });
    }

    JvmClass {
        version: JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: matches!(def.vis, Vis::Pub),
            is_interface: true,
            is_abstract: true,
            is_annotation: true,
            ..Default::default()
        },
        name: internal,
        super_class: JVM_OBJECT.to_string(),
        interfaces: vec!["java/lang/annotation/Annotation".to_string()],
        fields: vec![],
        methods,
        source_file,
        permitted_subclasses: vec![],
        is_record: false,
        bootstrap_methods: vec![],
        synthetic_methods: vec![],
        annotations,
    }
}

fn lower_data_class(
    hir: &Hir,
    def: &Def,
    data_def: &valen_hir::DataClassDef,
    typed_bodies: &IndexMap<DefId, TypedBody>,
    pkg: Option<&[SmolStr]>,
    source_file: Option<String>,
) -> JvmClass {
    let internal = class_internal_name(&def.name, pkg);
    let super_class = JVM_OBJECT.to_string();

    let fields: Vec<JvmField> = data_def
        .ctor_params
        .iter()
        .map(|p| lower_field(p, pkg, &hir.imports))
        .collect();

    let field_info: Vec<(String, JvmType)> = fields
        .iter()
        .map(|f| (f.name.clone(), f.ty.clone()))
        .collect();

    let mut methods = vec![
        generate_ctor(&internal, &super_class, &fields),
        data_class_methods::generate_equals(&internal, &field_info),
        data_class_methods::generate_hash_code(&internal, &field_info),
        data_class_methods::generate_to_string(&internal, &def.name, &field_info),
        data_class_methods::generate_copy(&internal, &field_info),
    ];

    let mut all_synthetic_lambdas = Vec::new();
    let mut all_bootstrap_methods = Vec::new();
    let mut interfaces = Vec::new();

    for impl_entry in &hir.trait_impls {
        if impl_entry.target_name == def.name {
            if let Some(trait_internal) = hir
                .imports
                .get(&impl_entry.trait_name)
                .map(|p| p.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/"))
            {
                interfaces.push(trait_internal);
            } else if is_sealed_trait_def(hir, &impl_entry.trait_name) {
                let iface = class_internal_name(&impl_entry.trait_name, pkg);
                if !interfaces.contains(&iface) {
                    interfaces.push(iface);
                }
            }
            for &mid in &impl_entry.methods {
                if let Some(method_def) = hir.defs.get(&mid) {
                    if let DefKind::Fn(fn_def) = &method_def.kind {
                        let body = typed_bodies.get(&mid);
                        let result = lower_method(hir, method_def, fn_def, body, &internal, pkg);
                        methods.push(result.method);
                        all_synthetic_lambdas.extend(result.synthetic_lambdas);
                        all_bootstrap_methods.extend(result.bootstrap_methods);
                    }
                }
            }
        }
    }

    // Collect methods from inherent impls, deduplicating by name
    if let Some(type_method_ids) = hir.type_methods.get(&def.name) {
        let existing_names: std::collections::HashSet<_> =
            methods.iter().map(|m| m.name.clone()).collect();
        for &mid in type_method_ids {
            if let Some(method_def) = hir.defs.get(&mid) {
                if let DefKind::Fn(fn_def) = &method_def.kind {
                    if existing_names.contains(method_def.name.as_str()) {
                        continue;
                    }
                    let body = typed_bodies.get(&mid);
                    let result = lower_method(hir, method_def, fn_def, body, &internal, pkg);
                    methods.push(result.method);
                    all_synthetic_lambdas.extend(result.synthetic_lambdas);
                    all_bootstrap_methods.extend(result.bootstrap_methods);
                }
            }
        }
    }

    let synthetic_methods = synthetic_lambdas_to_methods(all_synthetic_lambdas);

    JvmClass {
        version: JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: matches!(def.vis, Vis::Pub),
            is_final: true,
            is_super: true,
            ..Default::default()
        },
        name: internal,
        super_class,
        interfaces,
        fields,
        methods,
        source_file,
        permitted_subclasses: vec![],
        is_record: false,
        bootstrap_methods: all_bootstrap_methods,
        synthetic_methods,
        annotations: vec![],
    }
}

fn lower_field(
    param: &valen_hir::CtorParamDef,
    pkg: Option<&[SmolStr]>,
    imports: &IndexMap<SmolStr, Vec<SmolStr>>,
) -> JvmField {
    let ty = tyref_to_jvm(&param.ty, pkg, imports);
    let is_pub = matches!(param.vis, Vis::Pub);
    JvmField {
        access: JvmFieldAccess {
            is_public: is_pub,
            is_private: !is_pub,
            is_final: !param.mutable,
            ..Default::default()
        },
        name: param.name.to_string(),
        ty,
    }
}

fn generate_ctor(class_internal: &str, super_class: &str, fields: &[JvmField]) -> JvmMethod {
    let mut ops = Vec::new();

    // super.<init>()
    ops.push(JvmOp::LoadThis);
    ops.push(JvmOp::InvokeSpecial {
        owner: super_class.to_string(),
        name: INIT.to_string(),
        params: vec![],
        ret: JvmType::Void,
    });

    // this.field = param_n
    let mut slot = 1u16;
    for field in fields {
        ops.push(JvmOp::LoadThis);
        ops.push(JvmOp::LoadLocal(slot, field.ty.clone()));
        ops.push(JvmOp::PutField {
            owner: class_internal.to_string(),
            name: field.name.clone(),
            descriptor: field.ty.clone(),
        });
        slot += field.ty.slot_count();
    }

    ops.push(JvmOp::Return(JvmType::Void));

    let max_locals = 1 + fields.iter().map(|f| f.ty.slot_count()).sum::<u16>();

    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: INIT.to_string(),
        params: fields.iter().map(|f| f.ty.clone()).collect(),
        return_type: JvmType::Void,
        body: Some(JvmMethodBody {
            max_locals,
            ops,
            exception_handlers: vec![],
        }),
    }
}

fn generate_getter(class_internal: &str, field_name: &str, field_ty: &JvmType) -> JvmMethod {
    JvmMethod {
        access: JvmMethodAccess {
            is_public: true,
            ..Default::default()
        },
        name: field_name.to_string(),
        params: vec![],
        return_type: field_ty.clone(),
        body: Some(JvmMethodBody {
            max_locals: 1,
            ops: vec![
                JvmOp::LoadThis,
                JvmOp::GetField {
                    owner: class_internal.to_string(),
                    name: field_name.to_string(),
                    descriptor: field_ty.clone(),
                },
                JvmOp::Return(field_ty.clone()),
            ],
            exception_handlers: vec![],
        }),
    }
}

/// Result of lowering a single method, including any lambda artifacts.
struct LowerMethodResult {
    method: JvmMethod,
    synthetic_lambdas: Vec<SyntheticLambda>,
    bootstrap_methods: Vec<JvmBootstrapMethod>,
}

fn lower_method(
    hir: &Hir,
    def: &Def,
    fn_def: &FnDef,
    typed_body: Option<&TypedBody>,
    class_internal: &str,
    pkg: Option<&[SmolStr]>,
) -> LowerMethodResult {
    let params: Vec<JvmType> = fn_def
        .params
        .iter()
        .filter(|p| !p.is_self)
        .map(|p| tyref_to_jvm(&p.ty, pkg, &hir.imports))
        .collect();

    let return_type = fn_def
        .return_ty
        .as_ref()
        .map(|t| tyref_to_jvm(t, pkg, &hir.imports))
        .unwrap_or(JvmType::Void);

    let has_self = fn_def.params.iter().any(|p| p.is_self);

    let mut synthetic_lambdas = Vec::new();
    let mut bootstrap_methods = Vec::new();

    let body = if !fn_def.has_body {
        None
    } else if let Some(tb) = typed_body {
        let param_pairs: Vec<(SmolStr, JvmType)> = fn_def
            .params
            .iter()
            .filter(|p| !p.is_self)
            .map(|p| (p.name.clone(), tyref_to_jvm(&p.ty, pkg, &hir.imports)))
            .collect();
        let result = crate::expr::lower_body(
            tb,
            class_internal,
            &param_pairs,
            &return_type,
            has_self,
            pkg,
            hir,
        );
        synthetic_lambdas = result.synthetic_lambdas;
        bootstrap_methods = result.bootstrap_methods;
        Some(result.body)
    } else {
        let max_locals =
            (if has_self { 1u16 } else { 0 }) + params.iter().map(|t| t.slot_count()).sum::<u16>();
        Some(JvmMethodBody {
            max_locals,
            ops: crate::jvm_ir::throw_unsupported_ops("not yet implemented"),
            exception_handlers: vec![],
        })
    };

    LowerMethodResult {
        method: JvmMethod {
            access: JvmMethodAccess {
                is_public: matches!(def.vis, Vis::Pub),
                is_private: matches!(def.vis, Vis::Private),
                is_static: !has_self,
                is_abstract: !fn_def.has_body,
                ..Default::default()
            },
            name: def.name.to_string(),
            params,
            return_type,
            body,
        },
        synthetic_lambdas,
        bootstrap_methods,
    }
}

/// Converts collected `SyntheticLambda` entries into `JvmMethod` definitions.
fn synthetic_lambdas_to_methods(lambdas: Vec<SyntheticLambda>) -> Vec<JvmMethod> {
    lambdas
        .into_iter()
        .map(|lam| JvmMethod {
            access: JvmMethodAccess {
                is_private: true,
                is_static: true,
                is_synthetic: true,
                ..Default::default()
            },
            name: lam.name,
            params: lam.params,
            return_type: lam.return_type,
            body: Some(lam.body),
        })
        .collect()
}

fn class_access(vis: &Vis, kind: &ClassDefKind) -> JvmClassAccess {
    JvmClassAccess {
        is_public: matches!(vis, Vis::Pub),
        is_final: matches!(kind, ClassDefKind::Final),
        is_abstract: matches!(kind, ClassDefKind::Abstract | ClassDefKind::Sealed),
        is_super: true,
        ..Default::default()
    }
}

fn collect_permitted_subclasses(
    hir: &Hir,
    sealed_name: &str,
    pkg: Option<&[SmolStr]>,
) -> Vec<String> {
    let mut permitted = Vec::new();
    for (_id, def) in &hir.defs {
        match &def.kind {
            DefKind::Class(cd) => {
                if let Some(ref sup) = cd.superclass {
                    if superclass_matches(sup, sealed_name) {
                        permitted.push(class_internal_name(&def.name, pkg));
                    }
                }
            }
            DefKind::DataClass(_) => {
                // data class can extend sealed class — but DataClassDef has no superclass field yet
                // This will be handled when DataClassDef gains superclass support
            }
            _ => {}
        }
    }
    permitted
}

fn is_sealed_trait_def(hir: &Hir, name: &SmolStr) -> bool {
    hir.defs
        .values()
        .any(|d| d.name == *name && matches!(&d.kind, DefKind::Trait(t) if t.is_sealed))
}

fn superclass_matches(tyref: &valen_hir::TyRef, name: &str) -> bool {
    match tyref {
        valen_hir::TyRef::Named(n) => n.as_str() == name,
        valen_hir::TyRef::Generic(n, _) => n.as_str() == name,
        _ => false,
    }
}

fn lower_enum(
    hir: &Hir,
    def: &Def,
    enum_def: &valen_hir::EnumDef,
    typed_bodies: &IndexMap<DefId, TypedBody>,
    pkg: Option<&[SmolStr]>,
    source_file: Option<String>,
) -> Vec<JvmClass> {
    let enum_internal = class_internal_name(&def.name, pkg);
    let mut classes = Vec::new();

    let variant_internals: Vec<String> = enum_def
        .variants
        .iter()
        .map(|v| format!("{enum_internal}${}", v.name))
        .collect();

    let mut methods = Vec::new();
    let mut interfaces = Vec::new();
    let mut all_synthetic_lambdas = Vec::new();
    let mut all_bootstrap_methods = Vec::new();

    for impl_entry in &hir.trait_impls {
        if impl_entry.target_name == def.name {
            if let Some(trait_internal) = hir
                .imports
                .get(&impl_entry.trait_name)
                .map(|p| p.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/"))
            {
                if !interfaces.contains(&trait_internal) {
                    interfaces.push(trait_internal);
                }
            } else if is_sealed_trait_def(hir, &impl_entry.trait_name) {
                let iface = class_internal_name(&impl_entry.trait_name, pkg);
                if !interfaces.contains(&iface) {
                    interfaces.push(iface);
                }
            }
            for &mid in &impl_entry.methods {
                if let Some(method_def) = hir.defs.get(&mid) {
                    if let DefKind::Fn(fn_def) = &method_def.kind {
                        let body = typed_bodies.get(&mid);
                        let result =
                            lower_method(hir, method_def, fn_def, body, &enum_internal, pkg);
                        methods.push(result.method);
                        all_synthetic_lambdas.extend(result.synthetic_lambdas);
                        all_bootstrap_methods.extend(result.bootstrap_methods);
                    }
                }
            }
        }
    }

    // Collect methods from inherent impls
    if let Some(type_method_ids) = hir.type_methods.get(&def.name) {
        let existing_names: std::collections::HashSet<_> =
            methods.iter().map(|m| m.name.clone()).collect();
        for &mid in type_method_ids {
            if let Some(method_def) = hir.defs.get(&mid) {
                if let DefKind::Fn(fn_def) = &method_def.kind {
                    if existing_names.contains(method_def.name.as_str()) {
                        continue;
                    }
                    let body = typed_bodies.get(&mid);
                    let result = lower_method(hir, method_def, fn_def, body, &enum_internal, pkg);
                    methods.push(result.method);
                    all_synthetic_lambdas.extend(result.synthetic_lambdas);
                    all_bootstrap_methods.extend(result.bootstrap_methods);
                }
            }
        }
    }

    let synthetic_methods = synthetic_lambdas_to_methods(all_synthetic_lambdas);

    classes.push(JvmClass {
        version: JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: matches!(def.vis, Vis::Pub),
            is_abstract: true,
            is_interface: true,
            ..Default::default()
        },
        name: enum_internal.clone(),
        super_class: JVM_OBJECT.to_string(),
        interfaces,
        fields: vec![],
        methods,
        source_file: source_file.clone(),
        permitted_subclasses: variant_internals.clone(),
        is_record: false,
        bootstrap_methods: all_bootstrap_methods,
        synthetic_methods,
        annotations: vec![],
    });

    for (variant, variant_internal) in enum_def.variants.iter().zip(variant_internals.iter()) {
        if variant.fields.is_empty() {
            classes.push(lower_unit_variant(
                variant_internal,
                &enum_internal,
                source_file.clone(),
            ));
        } else {
            classes.push(lower_record_variant(
                variant_internal,
                &enum_internal,
                &variant.fields,
                &enum_def.derives,
                pkg,
                source_file.clone(),
                &hir.imports,
            ));
        }
    }

    classes
}

fn lower_record_variant(
    variant_internal: &str,
    enum_internal: &str,
    fields: &[(SmolStr, valen_hir::TyRef)],
    derives: &[SmolStr],
    pkg: Option<&[SmolStr]>,
    source_file: Option<String>,
    imports: &IndexMap<SmolStr, Vec<SmolStr>>,
) -> JvmClass {
    let jvm_fields: Vec<JvmField> = fields
        .iter()
        .map(|(name, tyref)| JvmField {
            access: JvmFieldAccess {
                is_private: true,
                is_final: true,
                ..Default::default()
            },
            name: name.to_string(),
            ty: tyref_to_jvm(tyref, pkg, imports),
        })
        .collect();

    let ctor = generate_ctor(variant_internal, JVM_RECORD, &jvm_fields);

    let mut methods = vec![ctor];
    for field in &jvm_fields {
        methods.push(generate_getter(variant_internal, &field.name, &field.ty));
    }

    let field_info: Vec<(String, JvmType)> = jvm_fields
        .iter()
        .map(|f| (f.name.clone(), f.ty.clone()))
        .collect();
    let variant_name_str = variant_internal
        .rsplit('$')
        .next()
        .unwrap_or(variant_internal);
    for d in derives {
        match d.as_str() {
            "Eq" => methods.push(data_class_methods::generate_equals(
                variant_internal,
                &field_info,
            )),
            "Hash" => methods.push(data_class_methods::generate_hash_code(
                variant_internal,
                &field_info,
            )),
            "Display" => methods.push(data_class_methods::generate_to_string(
                variant_internal,
                variant_name_str,
                &field_info,
            )),
            "Clone" => methods.push(data_class_methods::generate_copy(
                variant_internal,
                &field_info,
            )),
            _ => {}
        }
    }

    JvmClass {
        version: JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: true,
            is_final: true,
            is_super: true,
            ..Default::default()
        },
        name: variant_internal.to_string(),
        super_class: JVM_RECORD.to_string(),
        interfaces: vec![enum_internal.to_string()],
        fields: jvm_fields,
        methods,
        source_file,
        permitted_subclasses: vec![],
        is_record: true,
        bootstrap_methods: vec![],
        synthetic_methods: vec![],
        annotations: vec![],
    }
}

fn lower_unit_variant(
    variant_internal: &str,
    enum_internal: &str,
    source_file: Option<String>,
) -> JvmClass {
    let self_ty = JvmType::Object(variant_internal.to_string());

    let instance_field = JvmField {
        access: JvmFieldAccess {
            is_public: true,
            is_static: true,
            is_final: true,
            ..Default::default()
        },
        name: INSTANCE.to_string(),
        ty: self_ty.clone(),
    };

    let private_ctor = JvmMethod {
        access: JvmMethodAccess {
            is_private: true,
            ..Default::default()
        },
        name: INIT.to_string(),
        params: vec![],
        return_type: JvmType::Void,
        body: Some(JvmMethodBody {
            max_locals: 1,
            ops: vec![
                JvmOp::LoadThis,
                JvmOp::InvokeSpecial {
                    owner: JVM_OBJECT.to_string(),
                    name: INIT.to_string(),
                    params: vec![],
                    ret: JvmType::Void,
                },
                JvmOp::Return(JvmType::Void),
            ],
            exception_handlers: vec![],
        }),
    };

    let clinit = JvmMethod {
        access: JvmMethodAccess {
            is_static: true,
            ..Default::default()
        },
        name: CLINIT.to_string(),
        params: vec![],
        return_type: JvmType::Void,
        body: Some(JvmMethodBody {
            max_locals: 0,
            ops: vec![
                JvmOp::New(variant_internal.to_string()),
                JvmOp::Dup,
                JvmOp::InvokeSpecial {
                    owner: variant_internal.to_string(),
                    name: INIT.to_string(),
                    params: vec![],
                    ret: JvmType::Void,
                },
                JvmOp::PutStatic {
                    owner: variant_internal.to_string(),
                    name: INSTANCE.to_string(),
                    descriptor: self_ty,
                },
                JvmOp::Return(JvmType::Void),
            ],
            exception_handlers: vec![],
        }),
    };

    JvmClass {
        version: JvmVersion::Java21,
        access: JvmClassAccess {
            is_public: true,
            is_final: true,
            is_super: true,
            ..Default::default()
        },
        name: variant_internal.to_string(),
        super_class: JVM_OBJECT.to_string(),
        interfaces: vec![enum_internal.to_string()],
        fields: vec![instance_field],
        methods: vec![private_ctor, clinit],
        source_file,
        permitted_subclasses: vec![],
        is_record: false,
        bootstrap_methods: vec![],
        synthetic_methods: vec![],
        annotations: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valen_ast::FileId;
    use valen_hir::*;

    fn make_hir_with_class(
        name: &str,
        kind: ClassDefKind,
        params: Vec<CtorParamDef>,
        vis: Vis,
    ) -> Hir {
        let mut hir = Hir::default();
        let id = hir.alloc_id();
        hir.defs.insert(
            id,
            Def {
                id,
                name: SmolStr::from(name),
                kind: DefKind::Class(ClassDef {
                    kind,
                    ctor_params: params,
                    superclass: None,
                    trait_impls: vec![],
                    methods: vec![],
                }),
                vis,
                span: valen_ast::Span {
                    start: 0,
                    end: 0,
                    file_id: FileId(0),
                },
                package: None,
            },
        );
        hir
    }

    #[test]
    fn lower_empty_class() {
        let hir = make_hir_with_class("Foo", ClassDefKind::Final, vec![], Vis::Pub);
        let classes = lower_hir(&hir, &IndexMap::new());
        // User class + synthetic ListIterator
        let c = classes.iter().find(|c| c.name == "Foo").unwrap();
        assert_eq!(c.super_class, "java/lang/Object");
        assert!(c.access.is_public);
        assert!(c.access.is_final);
        assert!(c.fields.is_empty());
        assert_eq!(c.methods.len(), 1); // <init>
        assert_eq!(c.methods[0].name, "<init>");
    }

    #[test]
    fn lower_class_with_params() {
        let params = vec![
            CtorParamDef {
                vis: Vis::Pub,
                name: "name".into(),
                ty: TyRef::Prim(PrimTy::String),
                mutable: false,
                has_default: false,
            },
            CtorParamDef {
                vis: Vis::Private,
                name: "age".into(),
                ty: TyRef::Prim(PrimTy::Int),
                mutable: true,
                has_default: false,
            },
        ];
        let hir = make_hir_with_class("User", ClassDefKind::Final, params, Vis::Pub);
        let classes = lower_hir(&hir, &IndexMap::new());
        let c = classes
            .iter()
            .find(|c| c.name != "valen/core/ListIterator")
            .unwrap();
        assert_eq!(c.fields.len(), 2);

        assert_eq!(c.fields[0].name, "name");
        assert!(c.fields[0].access.is_public);
        assert!(c.fields[0].access.is_final);

        assert_eq!(c.fields[1].name, "age");
        assert!(c.fields[1].access.is_private);
        assert!(!c.fields[1].access.is_final); // mutable

        let ctor = &c.methods[0];
        assert_eq!(ctor.params.len(), 2);
    }

    #[test]
    fn lower_abstract_class() {
        let hir = make_hir_with_class("Shape", ClassDefKind::Abstract, vec![], Vis::Pub);
        let classes = lower_hir(&hir, &IndexMap::new());
        let c = classes
            .iter()
            .find(|c| c.name != "valen/core/ListIterator")
            .unwrap();
        assert!(c.access.is_abstract);
        assert!(!c.access.is_final);
    }

    #[test]
    fn lower_open_class() {
        let hir = make_hir_with_class("Animal", ClassDefKind::Open, vec![], Vis::Pub);
        let classes = lower_hir(&hir, &IndexMap::new());
        let c = classes
            .iter()
            .find(|c| c.name != "valen/core/ListIterator")
            .unwrap();
        assert!(!c.access.is_abstract);
        assert!(!c.access.is_final);
    }

    #[test]
    fn lower_data_class() {
        let mut hir = Hir::default();
        let id = hir.alloc_id();
        hir.defs.insert(
            id,
            Def {
                id,
                name: SmolStr::from("Point"),
                kind: DefKind::DataClass(DataClassDef {
                    derives: vec![],
                    ctor_params: vec![
                        CtorParamDef {
                            vis: Vis::Pub,
                            name: "x".into(),
                            ty: TyRef::Prim(PrimTy::Float),
                            mutable: false,
                            has_default: false,
                        },
                        CtorParamDef {
                            vis: Vis::Pub,
                            name: "y".into(),
                            ty: TyRef::Prim(PrimTy::Float),
                            mutable: false,
                            has_default: false,
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

        let classes = lower_hir(&hir, &IndexMap::new());
        let c = classes.iter().find(|c| c.name == "Point").unwrap();
        assert!(c.access.is_final);
        assert_eq!(c.fields.len(), 2);
        // <init>, equals, hashCode, toString, copy
        assert_eq!(c.methods.len(), 5);

        let method_names: Vec<&str> = c.methods.iter().map(|m| m.name.as_str()).collect();
        assert!(method_names.contains(&"<init>"));
        assert!(method_names.contains(&"equals"));
        assert!(method_names.contains(&"hashCode"));
        assert!(method_names.contains(&"toString"));
        assert!(method_names.contains(&"copy"));
    }

    #[test]
    fn lower_class_with_package() {
        let mut hir = make_hir_with_class("Foo", ClassDefKind::Final, vec![], Vis::Pub);
        hir.package = Some(vec!["com".into(), "example".into()]);
        let classes = lower_hir(&hir, &IndexMap::new());
        assert!(classes.iter().any(|c| c.name == "com/example/Foo"));
    }

    #[test]
    fn lower_class_with_method() {
        let mut hir = Hir::default();
        let class_id = hir.alloc_id();
        let method_id = hir.alloc_id();

        hir.defs.insert(
            method_id,
            Def {
                id: method_id,
                name: SmolStr::from("greet"),
                kind: DefKind::Fn(FnDef {
                    params: vec![ParamDef {
                        name: "self".into(),
                        ty: TyRef::SelfTy,
                        mutable: false,
                        is_self: true,
                        has_default: false,
                    }],
                    return_ty: Some(TyRef::Prim(PrimTy::String)),
                    has_body: true,
                    generic_bounds: vec![],
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

        hir.defs.insert(
            class_id,
            Def {
                id: class_id,
                name: SmolStr::from("User"),
                kind: DefKind::Class(ClassDef {
                    kind: ClassDefKind::Final,
                    ctor_params: vec![],
                    superclass: None,
                    trait_impls: vec![],
                    methods: vec![method_id],
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

        let classes = lower_hir(&hir, &IndexMap::new());
        let c = classes
            .iter()
            .find(|c| c.name != "valen/core/ListIterator")
            .unwrap();
        assert_eq!(c.methods.len(), 2); // <init> + greet
        let greet = &c.methods[1];
        assert_eq!(greet.name, "greet");
        assert!(greet.access.is_public);
        assert!(!greet.access.is_static); // has self
        assert!(greet.params.is_empty()); // self is not in JVM params
        assert!(greet.body.is_some());
        let body = greet.body.as_ref().unwrap();
        assert!(
            matches!(body.ops[0], JvmOp::New(_)),
            "expected throw ops for unimplemented body"
        );
    }

    #[test]
    fn lower_class_with_trait_impl() {
        let mut hir = Hir::default();
        let class_id = hir.alloc_id();
        let method_id = hir.alloc_id();

        hir.defs.insert(
            method_id,
            Def {
                id: method_id,
                name: SmolStr::from("greet"),
                kind: DefKind::Fn(FnDef {
                    params: vec![ParamDef {
                        name: "self".into(),
                        ty: TyRef::SelfTy,
                        mutable: false,
                        is_self: true,
                        has_default: false,
                    }],
                    return_ty: Some(TyRef::Prim(PrimTy::String)),
                    has_body: true,
                    generic_bounds: vec![],
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

        hir.defs.insert(
            class_id,
            Def {
                id: class_id,
                name: SmolStr::from("Dog"),
                kind: DefKind::Class(ClassDef {
                    kind: ClassDefKind::Final,
                    ctor_params: vec![],
                    superclass: None,
                    trait_impls: vec![TyRef::Named("Greeter".into())],
                    methods: vec![],
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

        hir.trait_impls.push(ImplEntry {
            trait_name: "Greeter".into(),
            target_name: "Dog".into(),
            methods: vec![method_id],
        });

        let classes = lower_hir(&hir, &IndexMap::new());
        let c = classes
            .iter()
            .find(|c| c.name != "valen/core/ListIterator")
            .unwrap();
        assert_eq!(c.name, "Dog");
        assert_eq!(c.interfaces, vec!["Greeter"]);
        assert_eq!(c.methods.len(), 2); // <init> + greet
        let greet = c.methods.iter().find(|m| m.name == "greet").unwrap();
        assert!(greet.access.is_public);
        assert!(!greet.access.is_static);
    }

    #[test]
    fn lower_enum_mixed_variants() {
        let mut hir = Hir::default();
        let id = hir.alloc_id();
        hir.defs.insert(
            id,
            Def {
                id,
                name: SmolStr::from("Shape"),
                kind: DefKind::Enum(EnumDef {
                    variants: vec![
                        EnumVariantDef {
                            name: "Circle".into(),
                            fields: vec![("r".into(), TyRef::Prim(PrimTy::Float))],
                        },
                        EnumVariantDef {
                            name: "Point".into(),
                            fields: vec![],
                        },
                    ],
                    derives: vec![],
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

        let classes = lower_hir(&hir, &IndexMap::new());

        // sealed interface
        let iface = classes.iter().find(|c| c.name == "Shape").unwrap();
        assert!(iface.access.is_interface);
        assert!(iface.access.is_abstract);
        assert_eq!(iface.permitted_subclasses.len(), 2);
        assert!(iface
            .permitted_subclasses
            .contains(&"Shape$Circle".to_string()));
        assert!(iface
            .permitted_subclasses
            .contains(&"Shape$Point".to_string()));

        // record variant
        let circle = classes.iter().find(|c| c.name == "Shape$Circle").unwrap();
        assert!(circle.access.is_final);
        assert!(circle.is_record);
        assert_eq!(circle.super_class, "java/lang/Record");
        assert_eq!(circle.interfaces, vec!["Shape"]);
        assert_eq!(circle.fields.len(), 1);
        assert_eq!(circle.fields[0].name, "r");
        assert!(circle.fields[0].access.is_private);
        assert!(circle.fields[0].access.is_final);
        assert_eq!(circle.methods.len(), 2); // <init> + getter for r

        // unit variant
        let point = classes.iter().find(|c| c.name == "Shape$Point").unwrap();
        assert!(point.access.is_final);
        assert!(!point.is_record);
        assert_eq!(point.super_class, "java/lang/Object");
        assert_eq!(point.interfaces, vec!["Shape"]);
        assert_eq!(point.fields.len(), 1); // INSTANCE
        assert_eq!(point.fields[0].name, "INSTANCE");
        assert!(point.fields[0].access.is_static);
        assert!(point.fields[0].access.is_public);
        assert_eq!(point.methods.len(), 2); // <init> + <clinit>
        assert!(point.methods[0].access.is_private); // private ctor
        assert!(point.methods[1].access.is_static); // clinit
    }

    #[test]
    fn lower_enum_with_package() {
        let mut hir = Hir::default();
        hir.package = Some(vec!["com".into(), "app".into()]);
        let id = hir.alloc_id();
        hir.defs.insert(
            id,
            Def {
                id,
                name: SmolStr::from("Color"),
                kind: DefKind::Enum(EnumDef {
                    variants: vec![EnumVariantDef {
                        name: "Red".into(),
                        fields: vec![],
                    }],
                    derives: vec![],
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

        let classes = lower_hir(&hir, &IndexMap::new());
        assert!(classes.iter().any(|c| c.name == "com/app/Color"));
        assert!(classes.iter().any(|c| c.name == "com/app/Color$Red"));
    }
}
