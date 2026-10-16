//! Counts and positions as the 32-bit numbers the protocol and the typed API carry: buffer and
//! triangle indices, element counts, edit indices, and lines and columns of a text.

/// A count or position as a protocol `u32`; values above `u32::MAX` saturate.
pub(crate) fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_conversions_saturate() {
        assert_eq!(saturating_u32(3), 3);
        assert_eq!(saturating_u32(usize::MAX), u32::MAX);
    }
}
