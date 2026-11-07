//! Card UI geometry (card spec V1.0, 2026-11-06).
//!
//! The grid card is one component for every item type: a fixed 40x40 icon
//! area holding a 32x32 image, and a single-line 12px title below it. The
//! red "!" badge sits 5px from the card's top-right corner and never takes
//! part in the layout — it is an absolute overlay.
//!
//! This module holds the badge hit test so the geometry can be unit-tested
//! instead of living inside the UI callback.

/// The badge is a 15px circle, 5px from the card's top and right edges.
pub const BADGE_SIZE: f32 = 15.0;
pub const BADGE_INSET: f32 = 5.0;

/// Whether the pointer is on the badge of a card.
///
/// `mouse_x`/`mouse_y` are relative to the card, `card_width` is the card's
/// width. A missing path is the only reason a badge exists, so a healthy
/// item never answers "on the badge".
pub fn badge_hit(path_missing: bool, mouse_x: f32, mouse_y: f32, card_width: f32) -> bool {
    if !path_missing
        || !mouse_x.is_finite()
        || !mouse_y.is_finite()
        || !card_width.is_finite()
        || card_width < BADGE_SIZE + BADGE_INSET
    {
        return false;
    }
    // Match the visible circular badge, not its transparent corner pixels.
    let center_x = card_width - BADGE_INSET - BADGE_SIZE / 2.0;
    let center_y = BADGE_INSET + BADGE_SIZE / 2.0;
    let dx = mouse_x - center_x;
    let dy = mouse_y - center_y;
    dx * dx + dy * dy <= (BADGE_SIZE / 2.0) * (BADGE_SIZE / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 卡片规范 V1.0 §5: the badge occupies the top-right corner, 5px from
    // both edges, 15px wide.
    #[test]
    fn badge_hit_covers_the_top_right_corner_only() {
        // The card is 88px wide: the badge spans x 68..83, y 5..20.
        assert!(badge_hit(true, 70.0, 10.0, 88.0));
        assert!(badge_hit(true, 83.0, 12.5, 88.0));
        assert!(badge_hit(true, 75.5, 20.0, 88.0));
        assert!(!badge_hit(true, 83.0, 20.0, 88.0), "transparent corner");
        assert!(!badge_hit(true, f32::NAN, 10.0, 88.0));
        assert!(!badge_hit(true, 75.0, f32::INFINITY, 88.0));
        assert!(!badge_hit(true, 75.0, 10.0, f32::INFINITY));
        assert!(!badge_hit(true, 0.0, 10.0, 10.0));
        // Just outside the badge.
        assert!(!badge_hit(true, 67.0, 10.0, 88.0));
        assert!(!badge_hit(true, 84.0, 10.0, 88.0));
        assert!(!badge_hit(true, 75.0, 4.0, 88.0));
        assert!(!badge_hit(true, 75.0, 21.0, 88.0));
        // A healthy item has no badge at all.
        assert!(!badge_hit(false, 70.0, 10.0, 88.0));
        // The pointer far away (the leave path) never hits.
        assert!(!badge_hit(true, -1000.0, -1000.0, 88.0));
    }
}
