use serde_json::Value;

pub(crate) struct PythonJsonFormatter;

pub(crate) struct Sorted<'v>(pub(crate) &'v Value);

pub(crate) struct PythonIndentFormatter {
    pub(super) indent: usize,
    pub(super) depth: usize,
    pub(super) has_value: bool,
}
