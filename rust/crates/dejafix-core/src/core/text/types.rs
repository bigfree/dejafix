pub struct PythonLines<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) start: usize,
    pub(super) number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DecimalKey {
    pub(super) length: usize,
    pub(super) digits: Vec<u8>,
}
