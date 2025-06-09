#[derive(Debug, Clone)]
pub struct SidePanel {
    pub show_right_panel: bool,
    pub min_panel_width: f32,
}

impl SidePanel {
    pub fn new(is_show: bool) -> Self {
        Self {
            show_right_panel: is_show,
            min_panel_width: 0.0,
        }
    }
}

impl SidePanel {
    pub const DEFAULT_WIDTH: f32 = 200.0;
    pub const MIN_WIDTH: f32 = 0.0;
    pub const MAX_WIDTH: f32 = 600.0;
    pub const CLOSE_WIDTH: f32 = 100.0;
}
