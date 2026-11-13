#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorRun {
    pub fix: bool,
    pub depth: i64,
    pub roots: Vec<String>,
    pub online: bool,
    pub auto: Option<bool>,
}
