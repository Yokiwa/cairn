//! Cairn core library: all launcher logic, independent of any UI.

pub mod error;
pub mod fsutil;
pub mod paths;
pub mod settings;

pub use error::{AppError, Result};
pub use paths::Paths;
