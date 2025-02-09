use egui::{Context, FontId};

use crate::types::Size;

#[derive(Debug, Clone)]
pub struct FontSettings {
    pub font_type: FontId,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            font_type: FontId::monospace(14.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalFont {
    font_type: FontId,
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self {
            font_type: FontSettings::default().font_type,
        }
    }
}

impl TerminalFont {
    pub fn new(settings: FontSettings) -> Self {
        Self {
            font_type: settings.font_type,
        }
    }

    pub fn font_size(&self) -> f32 {
        self.font_type.size
    }

    pub fn font_size_mut(&mut self) -> &mut f32 {
        &mut self.font_type.size
    }

    pub fn font_type(&self) -> FontId {
        self.font_type.clone()
    }

    pub fn font_measure(&self, ctx: &Context) -> Size {
        let (width, height) = ctx.fonts(|f| {
            (
                f.glyph_width(&self.font_type, 'M'),
                f.row_height(&self.font_type),
            )
        });

        Size::new(width, height)
    }
}
