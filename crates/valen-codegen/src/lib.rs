pub mod class_emit;
pub mod enum_emit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JvmVersion {
    Java21,
    Java25,
}

pub struct ClassFileOutput {
    pub internal_name: String,
    pub bytes: Vec<u8>,
}
