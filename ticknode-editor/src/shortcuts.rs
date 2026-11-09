//! Keyboard decisions independent of browser events.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    Delete,
    Escape,
    Fit,
    Undo,
    Redo,
    SelectAll,
    Duplicate,
    CreateFrame,
}

impl Shortcut {
    pub fn prevents_default(self) -> bool {
        matches!(self, Self::Undo | Self::Redo | Self::SelectAll | Self::Duplicate)
    }
}

pub fn shortcut(ctrl: bool, shift: bool, key: &str, pending_paste: bool) -> Option<Shortcut> {
    const KEYS: &[(bool, &str, Shortcut)] = &[
        (false, "delete", Shortcut::Delete), (false, "backspace", Shortcut::Delete),
        (false, "escape", Shortcut::Escape), (false, "f", Shortcut::Fit), (false, "а", Shortcut::Fit),
        (false, "c", Shortcut::CreateFrame), (false, "с", Shortcut::CreateFrame),
        (true, "z", Shortcut::Undo), (true, "я", Shortcut::Undo),
        (true, "y", Shortcut::Redo), (true, "н", Shortcut::Redo),
        (true, "a", Shortcut::SelectAll), (true, "ф", Shortcut::SelectAll),
        (true, "d", Shortcut::Duplicate), (true, "в", Shortcut::Duplicate),
    ];
    let key = key.to_lowercase();
    let action = KEYS.iter().find(|(modifier, binding, _)| *modifier == ctrl && *binding == key)?.2;
    if pending_paste && matches!(action, Shortcut::Undo | Shortcut::Redo | Shortcut::CreateFrame) {
        return None;
    }
    Some(if shift && action == Shortcut::Undo { Shortcut::Redo } else { action })
}

#[cfg(test)]
mod tests;
