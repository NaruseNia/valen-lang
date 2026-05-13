//! Platform-independent JVM IR. Lowered from typed HIR before final classfile emission.

/// Branch-target label identifier.
pub type Label = u32;

/// JVM type representation used throughout the IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JvmType {
    /// `byte` (8-bit signed).
    Byte,
    /// `short` (16-bit signed).
    Short,
    /// `int` (32-bit signed).
    Int,
    /// `long` (64-bit signed).
    Long,
    /// `float` (32-bit IEEE 754).
    Float,
    /// `double` (64-bit IEEE 754).
    Double,
    /// `char` (16-bit unsigned Unicode).
    Char,
    /// `boolean`.
    Boolean,
    /// `void` (return-only).
    Void,
    /// Reference type by internal name (e.g. `java/lang/String`).
    Object(String),
    /// Array of the given element type.
    Array(Box<JvmType>),
}

impl JvmType {
    /// Returns `true` for 64-bit types (`long`, `double`) that occupy two slots.
    pub fn is_wide(&self) -> bool {
        matches!(self, JvmType::Long | JvmType::Double)
    }

    /// Number of local-variable / stack slots this type occupies (1 or 2).
    pub fn slot_count(&self) -> u16 {
        if self.is_wide() {
            2
        } else {
            1
        }
    }

    /// Returns `true` for reference types (`Object`, `Array`).
    pub fn is_reference(&self) -> bool {
        matches!(self, JvmType::Object(_) | JvmType::Array(_))
    }

    /// Returns the boxed wrapper class internal name for a primitive type.
    pub fn boxed_name(prim: &JvmType) -> Option<&'static str> {
        match prim {
            JvmType::Int => Some("java/lang/Integer"),
            JvmType::Long => Some("java/lang/Long"),
            JvmType::Float => Some("java/lang/Float"),
            JvmType::Double => Some("java/lang/Double"),
            JvmType::Boolean => Some("java/lang/Boolean"),
            JvmType::Char => Some("java/lang/Character"),
            JvmType::Byte => Some("java/lang/Byte"),
            JvmType::Short => Some("java/lang/Short"),
            _ => None,
        }
    }
}

/// IR representation of a single JVM class, interface, or record.
#[derive(Debug, Clone)]
pub struct JvmClass {
    pub version: crate::JvmVersion,
    pub access: JvmClassAccess,
    /// JVM internal name (e.g. `com/example/Foo`).
    pub name: String,
    pub super_class: String,
    pub interfaces: Vec<String>,
    pub fields: Vec<JvmField>,
    pub methods: Vec<JvmMethod>,
    pub source_file: Option<String>,
    /// Sealed type permitted subclasses (for `PermittedSubclasses` attribute).
    pub permitted_subclasses: Vec<String>,
    /// Whether this class should carry the `Record` attribute.
    pub is_record: bool,
}

/// Access flags for a JVM class.
#[derive(Debug, Clone, Default)]
pub struct JvmClassAccess {
    pub is_public: bool,
    pub is_final: bool,
    pub is_abstract: bool,
    pub is_interface: bool,
    pub is_super: bool,
}

/// IR representation of a JVM field.
#[derive(Debug, Clone)]
pub struct JvmField {
    pub access: JvmFieldAccess,
    pub name: String,
    pub ty: JvmType,
}

/// Access flags for a JVM field.
#[derive(Debug, Clone, Default)]
pub struct JvmFieldAccess {
    pub is_public: bool,
    pub is_private: bool,
    pub is_protected: bool,
    pub is_final: bool,
    pub is_static: bool,
}

/// IR representation of a JVM method.
#[derive(Debug, Clone)]
pub struct JvmMethod {
    pub access: JvmMethodAccess,
    pub name: String,
    pub params: Vec<JvmType>,
    pub return_type: JvmType,
    pub body: Option<JvmMethodBody>,
}

/// Access flags for a JVM method.
#[derive(Debug, Clone, Default)]
pub struct JvmMethodAccess {
    pub is_public: bool,
    pub is_private: bool,
    pub is_protected: bool,
    pub is_static: bool,
    pub is_final: bool,
    pub is_abstract: bool,
    pub is_bridge: bool,
    pub is_synthetic: bool,
}

/// Method body containing local slot count and JVM operations.
#[derive(Debug, Clone)]
pub struct JvmMethodBody {
    pub max_locals: u16,
    pub ops: Vec<JvmOp>,
    /// Exception handlers for try-catch blocks (maps to the Code attribute's exception_table).
    pub exception_handlers: Vec<ExceptionHandler>,
}

/// JVM exception handler entry (metadata in the Code attribute's exception_table).
#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    /// Label marking the start of the try region (inclusive).
    pub start: Label,
    /// Label marking the end of the try region (exclusive).
    pub end: Label,
    /// Label marking the catch handler entry point.
    pub handler: Label,
    /// Internal name of the caught exception class, or `None` for catch-all (finally).
    pub catch_type: Option<String>,
}

/// A single JVM bytecode-level operation in the IR.
#[derive(Debug, Clone)]
pub enum JvmOp {
    LoadThis,
    LoadLocal(u16, JvmType),
    StoreLocal(u16, JvmType),

    GetField {
        owner: String,
        name: String,
        descriptor: JvmType,
    },
    PutField {
        owner: String,
        name: String,
        descriptor: JvmType,
    },
    GetStatic {
        owner: String,
        name: String,
        descriptor: JvmType,
    },
    PutStatic {
        owner: String,
        name: String,
        descriptor: JvmType,
    },

    InvokeSpecial {
        owner: String,
        name: String,
        params: Vec<JvmType>,
        ret: JvmType,
    },
    InvokeVirtual {
        owner: String,
        name: String,
        params: Vec<JvmType>,
        ret: JvmType,
    },
    InvokeStatic {
        owner: String,
        name: String,
        params: Vec<JvmType>,
        ret: JvmType,
    },
    InvokeInterface {
        owner: String,
        name: String,
        params: Vec<JvmType>,
        ret: JvmType,
    },

    New(String),
    Dup,
    Pop,
    Pop2,
    Swap,

    PushInt(i32),
    PushLong(i64),
    PushFloat(f32),
    PushDouble(f64),
    PushString(String),
    PushNull,

    Checkcast(String),
    Instanceof(String),

    Return(JvmType),

    Label(Label),
    Goto(Label),
    IfEq(Label),
    IfNe(Label),
    IfICmpEq(Label),
    IfICmpNe(Label),
    IfACmpEq(Label),
    IfACmpNe(Label),
    IfNull(Label),
    IfNonNull(Label),

    Arith(ArithOp, JvmType),
    Neg(JvmType),
    Cmp(CmpKind),
    Convert {
        from: JvmType,
        to: JvmType,
    },
    Bitwise(BitwiseOp, JvmType),

    IfLt(Label),
    IfGe(Label),
    IfGt(Label),
    IfLe(Label),
    IfICmpLt(Label),
    IfICmpGe(Label),
    IfICmpGt(Label),
    IfICmpLe(Label),

    AThrow,

    /// Declares the verification frame state at a branch target.
    /// Must appear immediately after a Label that is a branch target.
    Frame {
        locals: Vec<JvmType>,
        stack: Vec<JvmType>,
    },

    /// Placeholder body that emits `throw new UnsupportedOperationException`.
    StubBody,
}

/// Arithmetic binary operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// Floating-point / long comparison instruction kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpKind {
    /// `lcmp` — long compare.
    LCmp,
    /// `fcmpl` — float compare (NaN → -1).
    FCmpL,
    /// `fcmpg` — float compare (NaN → 1).
    FCmpG,
    /// `dcmpl` — double compare (NaN → -1).
    DCmpL,
    /// `dcmpg` — double compare (NaN → 1).
    DCmpG,
}

/// Bitwise / shift operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitwiseOp {
    And,
    Or,
    Xor,
    Shl,
    Shr,
    /// Unsigned (logical) right shift.
    UShr,
}

impl JvmOp {
    /// Returns the net stack-slot change produced by this operation.
    pub fn stack_delta(&self) -> i32 {
        match self {
            JvmOp::LoadThis => 1,
            JvmOp::LoadLocal(_, ty) => ty.slot_count() as i32,
            JvmOp::StoreLocal(_, ty) => -(ty.slot_count() as i32),
            JvmOp::GetField { descriptor, .. } => -1 + descriptor.slot_count() as i32, // pop receiver, push value
            JvmOp::PutField { descriptor, .. } => -1 - descriptor.slot_count() as i32, // pop receiver + value
            JvmOp::GetStatic { descriptor, .. } => descriptor.slot_count() as i32,
            JvmOp::PutStatic { descriptor, .. } => -(descriptor.slot_count() as i32),
            JvmOp::InvokeSpecial { params, ret, .. } | JvmOp::InvokeVirtual { params, ret, .. } => {
                let consumed: i32 = 1 + params.iter().map(|t| t.slot_count() as i32).sum::<i32>();
                let produced = if matches!(ret, JvmType::Void) {
                    0
                } else {
                    ret.slot_count() as i32
                };
                produced - consumed
            }
            JvmOp::InvokeStatic { params, ret, .. } => {
                let consumed: i32 = params.iter().map(|t| t.slot_count() as i32).sum();
                let produced = if matches!(ret, JvmType::Void) {
                    0
                } else {
                    ret.slot_count() as i32
                };
                produced - consumed
            }
            JvmOp::InvokeInterface { params, ret, .. } => {
                let consumed: i32 = 1 + params.iter().map(|t| t.slot_count() as i32).sum::<i32>();
                let produced = if matches!(ret, JvmType::Void) {
                    0
                } else {
                    ret.slot_count() as i32
                };
                produced - consumed
            }
            JvmOp::New(_) => 1,
            JvmOp::Dup => 1,
            JvmOp::Pop => -1,
            JvmOp::Pop2 => -2,
            JvmOp::Swap => 0,
            JvmOp::PushInt(_) | JvmOp::PushFloat(_) | JvmOp::PushString(_) | JvmOp::PushNull => 1,
            JvmOp::PushLong(_) | JvmOp::PushDouble(_) => 2,
            JvmOp::Checkcast(_) => 0,
            JvmOp::Instanceof(_) => 0, // pop ref, push int
            JvmOp::Return(ty) => {
                if matches!(ty, JvmType::Void) {
                    0
                } else {
                    -(ty.slot_count() as i32)
                }
            }
            JvmOp::Label(_) => 0,
            JvmOp::Goto(_) => 0,
            JvmOp::IfEq(_) | JvmOp::IfNe(_) | JvmOp::IfNull(_) | JvmOp::IfNonNull(_) => -1,
            JvmOp::IfICmpEq(_) | JvmOp::IfICmpNe(_) | JvmOp::IfACmpEq(_) | JvmOp::IfACmpNe(_) => -2,
            JvmOp::Arith(_, ty) | JvmOp::Bitwise(_, ty) => -(ty.slot_count() as i32),
            JvmOp::Neg(_) => 0,
            JvmOp::Cmp(kind) => {
                let operand_slots: i32 = match kind {
                    CmpKind::LCmp | CmpKind::DCmpL | CmpKind::DCmpG => 4,
                    CmpKind::FCmpL | CmpKind::FCmpG => 2,
                };
                1 - operand_slots
            }
            JvmOp::Convert { from, to } => to.slot_count() as i32 - from.slot_count() as i32,
            JvmOp::IfLt(_) | JvmOp::IfGe(_) | JvmOp::IfGt(_) | JvmOp::IfLe(_) => -1,
            JvmOp::IfICmpLt(_) | JvmOp::IfICmpGe(_) | JvmOp::IfICmpGt(_) | JvmOp::IfICmpLe(_) => -2,
            JvmOp::AThrow => -1,
            JvmOp::Frame { .. } => 0,
            JvmOp::StubBody => 0,
        }
    }
}
