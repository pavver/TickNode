//! Side panels (palette, inspector) and console.

mod console;
mod execution;
mod formatting;
mod inspector;
mod palette;
mod resources;
mod tooltip;
mod variables;

pub use console::*;
pub use formatting::{display_value, display_value_in, property_label, relative_time};
pub use inspector::*;
pub use palette::*;
pub use tooltip::*;
