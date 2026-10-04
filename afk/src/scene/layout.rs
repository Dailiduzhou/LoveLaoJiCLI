//! Reserve the garden before growing decorations. Never rely on leftover rows.
use super::clock;
use ratatui::layout::{Constraint, Layout, Margin, Rect};

pub(super) struct SceneLayout {
    pub(super) title: Rect,
    pub(super) clock: Rect,
    pub(super) caption: Rect,
    pub(super) field: Rect,
    pub(super) comfort: Rect,
    pub(super) notice: Rect,
    pub(super) controls: Rect,
    pub(super) banner: bool,
    pub(super) scale: u16,
}

impl SceneLayout {
    /// Called only for scene-sized viewports (at least 24 columns by 10 rows).
    pub(super) fn new(area: Rect, cells: usize, banner_width: u16) -> Self {
        let inner = area.inner(Margin::new(2, 1));
        // In short terminals remove optional prose, not plants or controls.
        let comfort_h = match area.height {
            18.. => 3,
            14.. => 2,
            _ => 0,
        };
        let [main, comfort, notice, controls] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(comfort_h),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        // Five rows fit a flower, leaves, stem and ground even at 24x10.
        // Taller terminals dedicate at least a third of their main area to grass.
        let garden_h = (main.height / 3).max(5);
        let banner = main.width >= banner_width
            && main.width >= clock::width(cells, 1)
            && main.height >= 8 + 1 + clock::height(1) + 1 + garden_h;
        let title_h = if banner {
            8
        } else {
            u16::from(main.height >= 7)
        };
        let budget = main.height.saturating_sub(title_h + 1 + 1 + garden_h);
        let scale = clock::fit(main.width, budget, cells);
        let gap = u16::from(scale != 0);
        let [title, _, clock, caption, field] = Layout::vertical([
            Constraint::Length(title_h),
            Constraint::Length(gap),
            Constraint::Length(clock::height(scale)),
            Constraint::Length(1),
            Constraint::Min(garden_h),
        ])
        .areas(main);
        Self {
            title,
            clock,
            caption,
            field,
            comfort,
            notice,
            controls,
            banner,
            scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_size_reserves_grass_and_fits_the_whole_clock() {
        for width in 24..=240 {
            for height in 10..=100 {
                for cells in [5, 8] {
                    let area = Rect::new(3, 2, width, height);
                    let plan = SceneLayout::new(area, cells, 48);
                    assert!(plan.field.height >= 5, "{width}x{height}, {cells} cells");
                    assert_eq!(plan.caption.height, 1);
                    assert_eq!(plan.controls.height, 1);
                    assert_eq!(plan.notice.height, 1);
                    assert!(plan.field.height >= (height.saturating_sub(7)) / 3);
                    if plan.scale != 0 {
                        assert_eq!(plan.clock.height, clock::height(plan.scale));
                        assert!(plan.clock.width >= clock::width(cells, plan.scale));
                    } else {
                        assert_eq!(plan.clock.height, 0);
                    }
                    let parts = [
                        plan.title,
                        plan.clock,
                        plan.caption,
                        plan.field,
                        plan.comfort,
                        plan.notice,
                        plan.controls,
                    ];
                    for part in parts {
                        assert_eq!(area.intersection(part), part);
                    }
                    for pair in parts.windows(2) {
                        assert!(pair[0].bottom() <= pair[1].y);
                    }
                }
            }
        }
    }

    #[test]
    fn common_sizes_do_not_sacrifice_the_garden_for_the_banner() {
        for (width, height) in [(80, 24), (120, 30), (60, 18), (240, 10), (24, 100)] {
            let plan = SceneLayout::new(Rect::new(0, 0, width, height), 5, 48);
            assert!(plan.field.height >= 5);
        }
        assert!(!SceneLayout::new(Rect::new(0, 0, 80, 24), 5, 48).banner);
        assert!(SceneLayout::new(Rect::new(0, 0, 80, 30), 5, 48).banner);
    }
}
