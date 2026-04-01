mod thread;
pub mod validator;
pub mod result;

pub use validator::{Validator, Validity};
pub use thread::run;
