mod doctor;
mod migrate;
mod types;

pub use doctor::{run as run_doctor, scan};
pub use migrate::run as run_migrate;
pub use types::DoctorRun;

#[cfg(test)]
mod tests;
