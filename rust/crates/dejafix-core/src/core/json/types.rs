use serde_json::Value;

pub(crate) struct PythonJsonFormatter;

pub(crate) struct Sorted<'v>(pub(crate) &'v Value);
