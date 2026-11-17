//! Editor glyphs that are not available in the Operator icon set.

use operator_ui::tokens::Icon;

use super::icons;

pub const BROOM: Icon = material("broom", &[icons::BROOM]);
pub const TERMINAL: Icon = material("terminal", &[icons::TERMINAL]);
pub const FIT: Icon = material("fit", &[icons::FIT]);
pub const THEME: Icon = material("theme", &[icons::THEME]);

const fn material(name: &'static str, paths: &'static [&'static str]) -> Icon {
    Icon {
        name,
        view_box: "0 0 24 24",
        paths,
    }
}
