//! Classpath scanning — reads Java .class files and extracts type metadata.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use ristretto_classfile::attributes::Attribute;
use ristretto_classfile::{ClassFile, MethodAccessFlags};
use smol_str::SmolStr;

use crate::{
    ForeignClassInfo, ForeignCtorInfo, ForeignFieldInfo, ForeignMethodInfo, PrimTy, TyRef,
};

/// Scans classpath entries for imported Java types and returns their metadata.
pub fn scan_classpath(
    classpath: &[PathBuf],
    imports: &IndexMap<SmolStr, Vec<SmolStr>>,
) -> IndexMap<SmolStr, ForeignClassInfo> {
    let mut result = IndexMap::new();

    for (short_name, path_segments) in imports {
        let internal_name: String = path_segments
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("/");

        if !internal_name.starts_with("java/")
            && !internal_name.starts_with("javax/")
            && !internal_name.starts_with("org/")
        {
            continue;
        }

        if let Some(info) = load_class_from_classpath(classpath, &internal_name) {
            result.insert(short_name.clone(), info);
        }
    }

    result
}

fn load_class_from_classpath(
    classpath: &[PathBuf],
    internal_name: &str,
) -> Option<ForeignClassInfo> {
    let relative = format!("{}.class", internal_name);

    for entry in classpath {
        let class_path = entry.join(&relative);
        if class_path.exists() {
            return load_class_file(&class_path, internal_name);
        }
    }
    None
}

fn load_class_file(path: &Path, internal_name: &str) -> Option<ForeignClassInfo> {
    let bytes = std::fs::read(path).ok()?;
    let cf = ClassFile::from_bytes(&bytes).ok()?;
    Some(extract_class_info(&cf, internal_name))
}

fn extract_class_info(cf: &ClassFile, internal_name: &str) -> ForeignClassInfo {
    let mut methods = Vec::new();
    let mut constructors = Vec::new();
    let mut fields = Vec::new();

    // Extract class-level type parameters from Signature attribute
    let type_params = extract_class_type_params(cf);

    for method in &cf.methods {
        let name = cf
            .constant_pool
            .try_get_utf8(method.name_index)
            .ok()
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let descriptor = cf
            .constant_pool
            .try_get_utf8(method.descriptor_index)
            .ok()
            .and_then(|s| s.as_str())
            .unwrap_or("");

        if name == "<clinit>" {
            continue;
        }

        let (params, ret) = parse_method_descriptor(descriptor);
        let is_static = method.access_flags.contains(MethodAccessFlags::STATIC);

        let generic_return_ty = extract_method_generic_return(cf, method, &type_params);

        if name == "<init>" {
            constructors.push(ForeignCtorInfo { params });
        } else {
            methods.push(ForeignMethodInfo {
                name: SmolStr::from(name),
                params,
                return_ty: ret,
                is_static,
                generic_return_ty,
            });
        }
    }

    for field in &cf.fields {
        let name = cf
            .constant_pool
            .try_get_utf8(field.name_index)
            .ok()
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let descriptor = cf
            .constant_pool
            .try_get_utf8(field.descriptor_index)
            .ok()
            .and_then(|s| s.as_str())
            .unwrap_or("");

        fields.push(ForeignFieldInfo {
            name: SmolStr::from(name),
            ty: parse_field_descriptor(descriptor),
        });
    }

    let super_class = cf
        .constant_pool
        .try_get_class(cf.super_class)
        .ok()
        .and_then(|s| s.as_str().map(|s| s.to_string()));

    let interfaces = cf
        .interfaces
        .iter()
        .filter_map(|&idx| {
            cf.constant_pool
                .try_get_class(idx)
                .ok()
                .and_then(|s| s.as_str().map(|s| s.to_string()))
        })
        .collect();

    let mut permitted_subclasses = Vec::new();
    let mut has_valen_closed = false;

    for attr in &cf.attributes {
        match attr {
            Attribute::PermittedSubclasses { class_indexes, .. } => {
                for &idx in class_indexes {
                    if let Ok(name) = cf.constant_pool.try_get_class(idx) {
                        if let Some(s) = name.as_str() {
                            permitted_subclasses.push(s.to_string());
                        }
                    }
                }
            }
            Attribute::RuntimeVisibleAnnotations { annotations, .. } => {
                for ann in annotations {
                    if let Ok(type_name) = cf.constant_pool.try_get_utf8(ann.type_index) {
                        if type_name.as_str() == Some("Lvalen/Closed;") {
                            has_valen_closed = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    ForeignClassInfo {
        internal_name: internal_name.to_string(),
        methods,
        constructors,
        fields,
        super_class,
        interfaces,
        permitted_subclasses,
        has_valen_closed,
        type_params,
    }
}

/// Extract class-level generic type parameter names from the Signature attribute.
/// E.g., for `HashMap<K,V>`, returns `["K", "V"]`.
fn extract_class_type_params(cf: &ClassFile) -> Vec<SmolStr> {
    for attr in &cf.attributes {
        if let Attribute::Signature {
            signature_index, ..
        } = attr
        {
            if let Ok(sig) = cf.constant_pool.try_get_utf8(*signature_index) {
                if let Some(s) = sig.as_str() {
                    return parse_type_params_from_signature(s);
                }
            }
        }
    }
    vec![]
}

/// Extract generic return type from a method's Signature attribute.
fn extract_method_generic_return(
    cf: &ClassFile,
    method: &ristretto_classfile::Method,
    class_type_params: &[SmolStr],
) -> Option<TyRef> {
    for attr in &method.attributes {
        if let Attribute::Signature {
            signature_index, ..
        } = attr
        {
            if let Ok(sig) = cf.constant_pool.try_get_utf8(*signature_index) {
                if let Some(s) = sig.as_str() {
                    return parse_generic_return_type(s, class_type_params);
                }
            }
        }
    }
    None
}

/// Parse type parameter names from a Java generic class signature.
/// Format: `<K:Ljava/lang/Object;V:Ljava/lang/Object;>...`
fn parse_type_params_from_signature(sig: &str) -> Vec<SmolStr> {
    let mut params = Vec::new();
    let mut chars = sig.chars().peekable();

    if chars.peek() != Some(&'<') {
        return params;
    }
    chars.next(); // consume '<'

    while chars.peek() != Some(&'>') && chars.peek().is_some() {
        // Read type parameter name (up to first ':')
        let mut name = String::new();
        while let Some(&ch) = chars.peek() {
            if ch == ':' {
                break;
            }
            name.push(ch);
            chars.next();
        }
        if !name.is_empty() {
            params.push(SmolStr::from(name));
        }
        // Skip class bound: `:Type`
        if chars.peek() == Some(&':') {
            chars.next(); // consume ':'
                          // Empty class bound (interface-only) — next char is ':' again
            if chars.peek() != Some(&':') && chars.peek() != Some(&'>') {
                skip_signature_type(&mut chars);
            }
        }
        // Skip interface bounds: `:Type` (zero or more)
        while chars.peek() == Some(&':') {
            chars.next();
            skip_signature_type(&mut chars);
        }
    }
    params
}

/// Parse the return type from a method signature, extracting type variable references.
/// Format: `(params)ReturnType` — we only care about the return type portion.
fn parse_generic_return_type(sig: &str, class_type_params: &[SmolStr]) -> Option<TyRef> {
    let mut chars = sig.chars().peekable();

    // Skip method-level type params if any: <...>
    if chars.peek() == Some(&'<') {
        let mut depth = 1;
        chars.next();
        while depth > 0 {
            match chars.next()? {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => {}
            }
        }
    }

    // Skip parameter types: (...)
    if chars.next() != Some('(') {
        return None;
    }
    while chars.peek() != Some(&')') && chars.peek().is_some() {
        skip_signature_type(&mut chars);
    }
    chars.next(); // consume ')'

    // Parse return type
    parse_signature_type(&mut chars, class_type_params)
}

/// Parse a single type from a Java generic signature string.
fn parse_signature_type(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    class_type_params: &[SmolStr],
) -> Option<TyRef> {
    match chars.peek()? {
        'T' => {
            chars.next(); // consume 'T'
            let mut name = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == ';' {
                    chars.next();
                    break;
                }
                name.push(ch);
                chars.next();
            }
            if class_type_params.iter().any(|p| p.as_str() == name) {
                Some(TyRef::Unresolved(SmolStr::from(name)))
            } else {
                // Method-level type param not in class scope — fall back to descriptor
                None
            }
        }
        'L' => {
            chars.next();
            let mut class_name = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == ';' || ch == '<' {
                    break;
                }
                class_name.push(ch);
                chars.next();
            }
            if chars.peek() == Some(&'<') {
                chars.next();
                let mut args = Vec::new();
                while chars.peek() != Some(&'>') && chars.peek().is_some() {
                    if let Some(arg) = parse_signature_type(chars, class_type_params) {
                        args.push(arg);
                    }
                }
                if chars.peek() == Some(&'>') {
                    chars.next();
                }
                if chars.peek() == Some(&';') {
                    chars.next();
                }
                let name = jvm_to_valen_name(&class_name);
                Some(TyRef::Generic(SmolStr::from(name), args))
            } else {
                if chars.peek() == Some(&';') {
                    chars.next();
                }
                let name = jvm_to_valen_name(&class_name);
                Some(TyRef::Named(SmolStr::from(name)))
            }
        }
        'B' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Byte))
        }
        'C' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Char))
        }
        'D' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Double))
        }
        'F' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Float))
        }
        'I' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Int))
        }
        'J' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Long))
        }
        'S' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Short))
        }
        'Z' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Bool))
        }
        'V' => {
            chars.next();
            Some(TyRef::Prim(PrimTy::Unit))
        }
        '[' => {
            chars.next();
            // Skip array element type — arrays not yet supported as Valen types
            skip_signature_type(chars);
            Some(TyRef::Named(SmolStr::from("Any")))
        }
        '*' => {
            chars.next();
            Some(TyRef::Named(SmolStr::from("Any")))
        }
        '+' | '-' => {
            chars.next();
            parse_signature_type(chars, class_type_params)
        }
        _ => None,
    }
}

/// Skip a type in a signature without parsing it.
fn skip_signature_type(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek() {
        Some('T') => while chars.next() != Some(';') {},
        Some('L') => {
            let mut depth = 0;
            chars.next();
            for ch in chars.by_ref() {
                match ch {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    ';' if depth == 0 => break,
                    _ => {}
                }
            }
        }
        Some('[') => {
            chars.next();
            skip_signature_type(chars);
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
}

fn jvm_to_valen_name(internal: &str) -> String {
    let short = internal.rsplit('/').next().unwrap_or(internal);
    match internal {
        "java/lang/String" => "String".to_string(),
        "java/lang/Integer" => "Int".to_string(),
        "java/lang/Long" => "Long".to_string(),
        "java/lang/Boolean" => "Bool".to_string(),
        _ => short.to_string(),
    }
}

fn parse_method_descriptor(desc: &str) -> (Vec<TyRef>, TyRef) {
    let mut chars = desc.chars().peekable();

    if chars.next() != Some('(') {
        return (vec![], TyRef::Error);
    }

    let mut params = Vec::new();
    while chars.peek() != Some(&')') && chars.peek().is_some() {
        params.push(parse_type_from_chars(&mut chars));
    }
    chars.next(); // consume ')'

    let ret = parse_type_from_chars(&mut chars);
    (params, ret)
}

fn parse_field_descriptor(desc: &str) -> TyRef {
    let mut chars = desc.chars().peekable();
    parse_type_from_chars(&mut chars)
}

fn parse_type_from_chars(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> TyRef {
    match chars.next() {
        Some('B') => TyRef::Prim(PrimTy::Byte),
        Some('C') => TyRef::Prim(PrimTy::Char),
        Some('D') => TyRef::Prim(PrimTy::Double),
        Some('F') => TyRef::Prim(PrimTy::Float),
        Some('I') => TyRef::Prim(PrimTy::Int),
        Some('J') => TyRef::Prim(PrimTy::Long),
        Some('S') => TyRef::Prim(PrimTy::Short),
        Some('Z') => TyRef::Prim(PrimTy::Bool),
        Some('V') => TyRef::Prim(PrimTy::Unit),
        Some('L') => {
            let mut class_name = String::new();
            for ch in chars.by_ref() {
                if ch == ';' {
                    break;
                }
                class_name.push(ch);
            }
            jvm_class_to_tyref(&class_name)
        }
        Some('[') => {
            let elem = parse_type_from_chars(chars);
            TyRef::Generic(SmolStr::from("Array"), vec![elem])
        }
        _ => TyRef::Error,
    }
}

fn jvm_class_to_tyref(internal: &str) -> TyRef {
    match internal {
        "java/lang/String" => TyRef::Prim(PrimTy::String),
        "java/lang/Object" => TyRef::Named(SmolStr::from("Object")),
        "java/lang/Integer" => TyRef::Prim(PrimTy::Int),
        "java/lang/Long" => TyRef::Prim(PrimTy::Long),
        "java/lang/Float" => TyRef::Prim(PrimTy::Float),
        "java/lang/Double" => TyRef::Prim(PrimTy::Double),
        "java/lang/Boolean" => TyRef::Prim(PrimTy::Bool),
        "java/lang/Character" => TyRef::Prim(PrimTy::Char),
        "java/lang/Byte" => TyRef::Prim(PrimTy::Byte),
        "java/lang/Short" => TyRef::Prim(PrimTy::Short),
        "java/lang/Void" => TyRef::Prim(PrimTy::Unit),
        other => {
            let short = other.rsplit('/').next().unwrap_or(other);
            TyRef::Named(SmolStr::from(short))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_void_method() {
        let (params, ret) = parse_method_descriptor("()V");
        assert!(params.is_empty());
        assert_eq!(ret, TyRef::Prim(PrimTy::Unit));
    }

    #[test]
    fn parse_int_to_string() {
        let (params, ret) = parse_method_descriptor("(I)Ljava/lang/String;");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], TyRef::Prim(PrimTy::Int));
        assert_eq!(ret, TyRef::Prim(PrimTy::String));
    }

    #[test]
    fn parse_object_params() {
        let (params, ret) = parse_method_descriptor("(Ljava/lang/Object;)Z");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], TyRef::Named(SmolStr::from("Object")));
        assert_eq!(ret, TyRef::Prim(PrimTy::Bool));
    }

    #[test]
    fn parse_multiple_params() {
        let (params, ret) = parse_method_descriptor("(IJ)D");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], TyRef::Prim(PrimTy::Int));
        assert_eq!(params[1], TyRef::Prim(PrimTy::Long));
        assert_eq!(ret, TyRef::Prim(PrimTy::Double));
    }

    #[test]
    fn parse_field_object() {
        let ty = parse_field_descriptor("Ljava/util/List;");
        assert_eq!(ty, TyRef::Named(SmolStr::from("List")));
    }

    #[test]
    fn signature_class_type_params() {
        let params = parse_type_params_from_signature(
            "<K:Ljava/lang/Object;V:Ljava/lang/Object;>Ljava/util/AbstractMap<TK;TV;>;",
        );
        assert_eq!(params, vec![SmolStr::from("K"), SmolStr::from("V")]);
    }

    #[test]
    fn signature_single_type_param() {
        let params =
            parse_type_params_from_signature("<E:Ljava/lang/Object;>Ljava/util/AbstractList<TE;>;");
        assert_eq!(params, vec![SmolStr::from("E")]);
    }

    #[test]
    fn signature_no_type_params() {
        let params = parse_type_params_from_signature("Ljava/lang/Object;");
        assert!(params.is_empty());
    }

    #[test]
    fn signature_generic_return_type_var() {
        let class_params = vec![SmolStr::from("K"), SmolStr::from("V")];
        let ret = parse_generic_return_type("(Ljava/lang/Object;)TV;", &class_params);
        assert_eq!(ret, Some(TyRef::Unresolved(SmolStr::from("V"))));
    }

    #[test]
    fn signature_generic_return_concrete() {
        let class_params = vec![SmolStr::from("K"), SmolStr::from("V")];
        let ret = parse_generic_return_type("()Ljava/util/Set<TK;>;", &class_params);
        assert_eq!(
            ret,
            Some(TyRef::Generic(
                SmolStr::from("Set"),
                vec![TyRef::Unresolved(SmolStr::from("K"))]
            ))
        );
    }

    #[test]
    fn signature_generic_return_primitive() {
        let class_params = vec![SmolStr::from("T")];
        let ret = parse_generic_return_type("()I", &class_params);
        assert_eq!(ret, Some(TyRef::Prim(PrimTy::Int)));
    }
}
