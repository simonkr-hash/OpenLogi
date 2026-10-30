//! The G502 X Plus drawing and its button layout.
//!
//! Everything model-specific lives here so upstream's mouse view stays
//! untouched apart from two hand-offs: [`super::hotspots::spy_overlay_for`]
//! reads [`OMIT`] and [`hotspots`], and the view draws [`G502Silhouette`]
//! when the model has no cached render. The drawing and the hotspots share one
//! coordinate system ([`MOUSE_MODEL_SIZE`] units); change them together.
//!
//! Physical layout (top-down, thumb rest on the left), front to back on the
//! left edge: G8 (DPI up), G7 (DPI down), the DPI-shift key, G5 (forward), G4
//! (back). The wheel tilts left/right and clicks; a free-spin lock switch sits
//! under it and sends no input, so it has no hotspot. The profile button is
//! below that switch.

use gpui::{
    App, IntoElement, ParentElement, PathBuilder, RenderOnce, Styled, Window, canvas, div, hsla,
    point, px,
};

use super::hotspots::{Hotspot, MOUSE_MODEL_SIZE, MouseControlId};
use crate::ui::theme;
use openlogi_core::binding::ButtonId;

/// Everything the G502 overlay replaces: the MX-only slots plus the generic
/// silhouette's back/forward/wheel slots, which [`hotspots`] re-places.
pub(super) const OMIT: [MouseControlId; 6] = [
    MouseControlId::Button(ButtonId::GestureButton),
    MouseControlId::Button(ButtonId::DpiToggle),
    MouseControlId::Button(ButtonId::HapticPanel),
    MouseControlId::Button(ButtonId::Back),
    MouseControlId::Button(ButtonId::Forward),
    MouseControlId::Button(ButtonId::MiddleClick),
];

/// The MX-only controls the G502 does not have (a subset of [`OMIT`]).
#[cfg(test)]
pub(super) const MX_ONLY: [MouseControlId; 3] = [
    MouseControlId::Button(ButtonId::GestureButton),
    MouseControlId::Button(ButtonId::DpiToggle),
    MouseControlId::Button(ButtonId::HapticPanel),
];

/// Button zones in [`MOUSE_MODEL_SIZE`] units.
pub(super) fn hotspots() -> Vec<Hotspot> {
    let spot = |id: ButtonId, x: f32, y: f32, w: f32, h: f32| Hotspot {
        id: id.into(),
        x,
        y,
        w,
        h,
    };
    vec![
        spot(ButtonId::DpiUp, 62., 70., 52., 46.),
        spot(ButtonId::DpiDown, 62., 122., 52., 46.),
        spot(ButtonId::DpiShift, 58., 188., 56., 48.),
        spot(ButtonId::Forward, 56., 262., 62., 50.),
        spot(ButtonId::Back, 56., 318., 62., 50.),
        spot(ButtonId::WheelTiltLeft, 170., 62., 29., 86.),
        spot(ButtonId::MiddleClick, 199., 62., 24., 86.),
        spot(ButtonId::WheelTiltRight, 223., 62., 29., 86.),
        spot(ButtonId::ProfileCycle, 196., 194., 30., 36.),
    ]
}

/// Top-down outline of the G502 X Plus as cubic Béziers in
/// [`MOUSE_MODEL_SIZE`] units: `[start, c1, c2, end]` per segment, clockwise
/// from the front-left. Straight edges are degenerate cubics.
const G502_BODY: [[(f32, f32); 4]; 9] = [
    [(118., 30.), (150., 8.), (270., 8.), (305., 30.)],
    [(305., 30.), (330., 60.), (345., 200.), (340., 330.)],
    [(340., 330.), (335., 440.), (310., 520.), (245., 545.)],
    [(245., 545.), (190., 556.), (140., 535.), (120., 480.)],
    [(120., 480.), (108., 440.), (112., 400.), (100., 380.)],
    [(100., 380.), (70., 372.), (52., 350.), (52., 320.)],
    [(52., 320.), (52., 300.), (52., 270.), (52., 250.)],
    [(52., 250.), (52., 228.), (70., 218.), (96., 214.)],
    [(96., 214.), (108., 120.), (108., 70.), (118., 30.)],
];

/// The body outline as a closed polyline in model units.
fn g502_outline() -> Vec<(f32, f32)> {
    const STEPS: u8 = 14;
    let mut points = Vec::new();
    for [p0, p1, p2, p3] in &G502_BODY {
        for step in 0..STEPS {
            let t = f32::from(step) / f32::from(STEPS);
            let u = 1. - t;
            let weights = [u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t];
            let at = |pick: fn(&(f32, f32)) -> f32| {
                weights[0] * pick(p0)
                    + weights[1] * pick(p1)
                    + weights[2] * pick(p2)
                    + weights[3] * pick(p3)
            };
            points.push((at(|p| p.0), at(|p| p.1)));
        }
    }
    points
}

/// Top-down drawing of the G502 X Plus, authored in [`MOUSE_MODEL_SIZE`] units
/// and scaled to the canvas. The button zones line up with
/// [`super::hotspots::spy_hotspots`]; move them together.
#[derive(IntoElement)]
pub(super) struct G502Silhouette {
    pub(super) w: f32,
    pub(super) h: f32,
}

impl RenderOnce for G502Silhouette {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pal = theme::palette(cx);
        let s = self.w / MOUSE_MODEL_SIZE.0;
        let dark = hsla(0., 0., 0.25, 1.0);
        let key = hsla(0., 0., 0.07, 1.0);
        // One rounded block in model units: `(x, y, w, h, corner radius)`.
        let block = move |(x, y, w, h, r): (f32, f32, f32, f32, f32)| {
            div()
                .absolute()
                .left(px(x * s))
                .top(px(y * s))
                .w(px(w * s))
                .h(px(h * s))
                .rounded(px(r * s))
        };
        let (fill, stroke) = (pal.muted, pal.text_muted);
        let shell = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let origin = bounds.origin;
                let to_screen =
                    |(x, y): (f32, f32)| point(origin.x + px(x * s), origin.y + px(y * s));
                let outline: Vec<_> = g502_outline().into_iter().map(to_screen).collect();
                let mut body = PathBuilder::fill();
                body.add_polygon(&outline, true);
                if let Ok(path) = body.build() {
                    window.paint_path(path, fill);
                }
                let mut edge = PathBuilder::stroke(px(1.5));
                edge.add_polygon(&outline, true);
                if let Ok(path) = edge.build() {
                    window.paint_path(path, stroke);
                }
            },
        )
        .size_full();
        div()
            .absolute()
            .inset_0()
            .w(px(self.w))
            .h(px(self.h))
            .child(shell)
            // Left and right click plates with the wheel between them.
            .child(block((112., 22., 91., 193., 30.)).border_1().border_color(pal.border))
            .child(block((219., 22., 93., 193., 30.)).border_1().border_color(pal.border))
            .child(block((199., 62., 24., 86., 12.)).bg(key))
            // Free-spin lock under the wheel: a switch, not a button.
            .child(block((201., 156., 20., 30., 7.)).bg(dark))
            // Profile button.
            .child(block((196., 194., 30., 36., 10.)).bg(key))
            // Left edge, front to back: G8, G7, DPI shift, G5, G4.
            .child(block((62., 70., 52., 46., 10.)).bg(key))
            .child(block((62., 122., 52., 46., 10.)).bg(key))
            .child(block((58., 188., 56., 48., 12.)).bg(key))
            .child(block((56., 262., 62., 50., 12.)).bg(key))
            .child(block((56., 318., 62., 50., 12.)).bg(key))
            // Battery / mode LED.
            .child(block((124., 42., 10., 36., 5.)).bg(hsla(0.4, 0.7, 0.5, 1.0)))
    }
}
