mod casefold;
mod casefold_table;
mod python;
mod types;

pub use casefold::casefold;
#[cfg(any(test, feature = "testing"))]
pub use casefold::casefold_table;
pub use python::{
    PYTHON_SPACE_CLASS, char_len, char_prefix, collapse_whitespace, contains_word, decimal_value,
    is_python_decimal, is_python_space, is_python_word, platform_newlines, python_lines,
    python_split, python_strip, python_strip_matches, universal_newlines, zfill,
};
pub use types::DecimalKey;

#[cfg(test)]
mod tests;
