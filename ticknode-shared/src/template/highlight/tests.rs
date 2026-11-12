use super::*;

pub(super) fn assert_highlights_invariant(source: &str, highlights: &[Highlight]) {
    if source.is_empty() {
        assert!(highlights.is_empty());
        return;
    }
    assert!(!highlights.is_empty());
    assert_eq!(highlights[0].range.start, 0);
    assert_eq!(highlights.last().unwrap().range.end, source.len());

    for i in 0..highlights.len() {
        let h = &highlights[i];
        assert!(
            h.range.start < h.range.end,
            "range must not be empty at index {i}: {:?}",
            h.range
        );
        assert!(
            source.is_char_boundary(h.range.start),
            "start {} must be a char boundary",
            h.range.start
        );
        assert!(
            source.is_char_boundary(h.range.end),
            "end {} must be a char boundary",
            h.range.end
        );

        if i > 0 {
            let prev = &highlights[i - 1];
            assert_eq!(
                prev.range.end, h.range.start,
                "adjacent ranges must be contiguous: {:?} then {:?}",
                prev, h
            );
            assert_ne!(
                prev.class, h.class,
                "adjacent ranges of identical class must be merged: {:?} then {:?}",
                prev, h
            );
        }
    }
}

mod expressions;
mod literals;
