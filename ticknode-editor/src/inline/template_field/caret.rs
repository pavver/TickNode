//! Caret UTF-16 / UTF-8 conversion, completion splicing, and DOM mirror measurement.

use std::ops::Range;

/// Converts a UTF-16 code unit offset into a UTF-8 byte offset in `text`.
/// Clamps to `text.len()`. If the offset lands inside a surrogate pair, aligns to char boundary.
pub fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut current_u16 = 0;
    for (byte_offset, ch) in text.char_indices() {
        if current_u16 >= utf16_offset {
            return byte_offset;
        }
        let next_u16 = current_u16 + ch.len_utf16();
        if next_u16 > utf16_offset {
            return byte_offset;
        }
        current_u16 = next_u16;
    }
    text.len()
}

/// Converts a UTF-8 byte offset into a UTF-16 code unit offset in `text`.
/// Clamps to total UTF-16 length. If `byte_offset` lands within a multibyte char, aligns to char boundary.
pub fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    let mut current_u16 = 0;
    for (b_offset, ch) in text.char_indices() {
        if b_offset >= byte_offset {
            return current_u16;
        }
        if b_offset + ch.len_utf8() > byte_offset {
            return current_u16;
        }
        current_u16 += ch.len_utf16();
    }
    current_u16
}

/// Replaces `range` with `insert` in `source`, returning the updated string and caret byte offset.
/// If `insert` ends with `()` (e.g. functions), the caret is placed between the parentheses.
pub fn apply_completion(source: &str, range: Range<usize>, insert: &str) -> (String, usize) {
    let start = range.start.min(source.len());
    let end = range.end.min(source.len()).max(start);
    let mut result = String::with_capacity(source.len() + insert.len() - (end - start));
    result.push_str(&source[..start]);
    result.push_str(insert);
    result.push_str(&source[end..]);

    let caret_byte = if insert.ends_with("()") {
        start + insert.len() - 1
    } else {
        start + insert.len()
    };
    (result, caret_byte)
}

/// Maps rectangles measured in a mirror div onto the screen position of the (possibly zoomed) textarea.
#[cfg(target_arch = "wasm32")]
pub struct MirrorFrame {
    origin_x: f64,
    origin_y: f64,
    div_left: f64,
    div_top: f64,
    /// Canvas zoom: the textarea is CSS-transformed while the mirror in `<body>` is not.
    scale: f64,
}

#[cfg(target_arch = "wasm32")]
impl MirrorFrame {
    /// Screen `(x, y, width, height)` of a rectangle measured inside the mirror.
    pub fn screen_rect(&self, rect: &web_sys::DomRect) -> (f64, f64, f64, f64) {
        (
            self.origin_x + (rect.left() - self.div_left) * self.scale,
            self.origin_y + (rect.top() - self.div_top) * self.scale,
            rect.width() * self.scale,
            rect.height() * self.scale,
        )
    }
}

/// A hidden, unscaled copy of the textarea's box and typography appended to `<body>`.
/// Fill it with content, call [`Mirror::frame`], measure, then [`Mirror::remove`].
#[cfg(target_arch = "wasm32")]
pub struct Mirror {
    pub div: web_sys::Element,
    pub document: web_sys::Document,
    computed: web_sys::CssStyleDeclaration,
}

#[cfg(target_arch = "wasm32")]
impl Mirror {
    pub fn new(textarea: &web_sys::HtmlTextAreaElement) -> Option<Self> {
        use wasm_bindgen::JsCast;
        let window = web_sys::window()?;
        let document = window.document()?;
        let computed = window.get_computed_style(textarea).ok().flatten()?;
        let div = document.create_element("div").ok()?;
        let style = div.dyn_ref::<web_sys::HtmlElement>()?.style();
        for (name, value) in [
            ("position", "absolute"),
            ("visibility", "hidden"),
            ("top", "0"),
            ("left", "0"),
            ("white-space", "pre-wrap"),
            ("word-wrap", "break-word"),
            ("overflow-wrap", "break-word"),
            ("overflow", "hidden"),
            ("border-style", "solid"),
            ("border-color", "transparent"),
        ] {
            let _ = style.set_property(name, value);
        }
        for name in [
            "box-sizing",
            "width",
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "letter-spacing",
            "line-height",
            "tab-size",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ] {
            let _ =
                style.set_property(name, &computed.get_property_value(name).unwrap_or_default());
        }
        document.body()?.append_child(&div).ok()?;
        Some(Self {
            div,
            document,
            computed,
        })
    }

    /// Call after the content is appended: relates mirror coordinates to the textarea on screen.
    pub fn frame(&self, textarea: &web_sys::HtmlTextAreaElement) -> MirrorFrame {
        let _ = &self.computed;
        let ta_rect = textarea.get_bounding_client_rect();
        let div_rect = self.div.get_bounding_client_rect();
        let width = textarea.offset_width() as f64;
        let scale = if width > 0.0 {
            ta_rect.width() / width
        } else {
            1.0
        };
        MirrorFrame {
            origin_x: ta_rect.left() - textarea.scroll_left() as f64 * scale,
            origin_y: ta_rect.top() - textarea.scroll_top() as f64 * scale,
            div_left: div_rect.left(),
            div_top: div_rect.top(),
            scale,
        }
    }

    pub fn remove(self) {
        self.div.remove();
    }
}

/// Computes screen coordinates of the caret in a `<textarea>` using a mirror div.
/// Returns `(caret_x, caret_bottom_y, caret_top_y)`.
#[cfg(target_arch = "wasm32")]
pub fn measure_caret_screen_pos(
    textarea: &web_sys::HtmlTextAreaElement,
    caret_byte: usize,
    source: &str,
) -> Option<(f64, f64, f64)> {
    let mirror = Mirror::new(textarea)?;
    let text_before = source.get(..caret_byte).unwrap_or(source);
    let _ = mirror
        .div
        .append_child(&mirror.document.create_text_node(text_before));
    let marker = mirror.document.create_element("span").ok()?;
    marker.set_text_content(Some("\u{200b}"));
    let _ = mirror.div.append_child(&marker);
    let frame = mirror.frame(textarea);
    let (x, top, _, height) = frame.screen_rect(&marker.get_bounding_client_rect());
    mirror.remove();
    Some((x, top + height.max(16.0 * frame.scale), top))
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn measure_caret_screen_pos(
    _textarea: &web_sys::HtmlTextAreaElement,
    _caret_byte: usize,
    _source: &str,
) -> Option<(f64, f64, f64)> {
    None
}
