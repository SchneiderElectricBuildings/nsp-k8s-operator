mod macros;

pub mod definitions;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    Info,
    Warning,
    Error,
}
