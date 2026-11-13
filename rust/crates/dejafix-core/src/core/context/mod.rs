mod generate;
mod sections;
mod types;

pub use generate::generate_context;
pub use types::ContextInput;

#[cfg(test)]
mod tests;
