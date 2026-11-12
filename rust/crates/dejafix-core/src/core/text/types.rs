pub struct PythonLines<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) start: usize,
    pub(super) number: usize,
}
