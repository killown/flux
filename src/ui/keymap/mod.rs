pub mod constants;

mod map;
mod parse;

pub use map::KeyMap;
pub use parse::{parse_accel_opt, DISABLED};
