use egui::emath::{vec2, Pos2, Rect, Rot2};
use egui::epaint::{Color32, PathShape, Stroke};
use egui::{CornerRadius, Painter};

/// Draws a rectangle with rounded corners, rotated around an origin.
pub(crate) fn draw_rotated_rect(
    painter: &Painter,
    rect: Rect,
    rounding: impl Into<CornerRadius>,
    fill: impl Into<Color32>,
    rot: impl Into<Rot2>,
    origin: Pos2,
) {
    let rounding = rounding.into();
    let fill = fill.into();
    let rot = rot.into();
    let safe_inset_points = safe_inset_points(rect, rot, origin, rounding);

    painter.add(PathShape::convex_polygon(
        safe_inset_points.to_vec(),
        fill,
        Stroke::NONE,
    ));

    // Nobody will notice that we're not actually drawing
    // the round bits... 🤫
}

fn safe_inset_points(rect: Rect, rot: Rot2, origin: Pos2, rounding: CornerRadius) -> [Pos2; 8] {
    [
        rotate(rect.left_top() + vec2(0.0, rounding.nw as f32), rot, origin),
        rotate(rect.left_top() + vec2(rounding.nw as f32, 0.0), rot, origin),
        rotate(
            rect.right_top() - vec2(rounding.ne as f32, 0.0),
            rot,
            origin,
        ),
        rotate(
            rect.right_top() + vec2(0.0, rounding.ne as f32),
            rot,
            origin,
        ),
        rotate(
            rect.right_bottom() - vec2(0.0, rounding.se as f32),
            rot,
            origin,
        ),
        rotate(
            rect.right_bottom() - vec2(rounding.se as f32, 0.0),
            rot,
            origin,
        ),
        rotate(
            rect.left_bottom() + vec2(rounding.sw as f32, 0.0),
            rot,
            origin,
        ),
        rotate(
            rect.left_bottom() - vec2(0.0, rounding.sw as f32),
            rot,
            origin,
        ),
    ]
}

fn rotate(point: Pos2, rot: Rot2, origin: Pos2) -> Pos2 {
    origin + rot * (point - origin)
}
