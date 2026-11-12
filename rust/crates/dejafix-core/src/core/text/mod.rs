mod python;
mod types;

#[cfg(any(test, feature = "testing"))]
pub use python::is_python_space;
pub use python::{is_python_decimal, python_lines, python_strip, python_strip_matches, zfill};

#[cfg(test)]
mod tests;
