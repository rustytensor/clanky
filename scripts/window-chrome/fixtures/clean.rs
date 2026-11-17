// A fixture for the design gate's source scan: tokens only, one exempted literal, no hits.
use gpui::{div, px, IntoElement, Styled};

mod tokens;

pub fn card(width: f32) -> impl IntoElement {
    div()
        .h(tokens::CARD_HEIGHT)
        .rounded(tokens::RADIUS_MEDIUM)
        .w(px(width / 2.0))
        .w(px(40.)) // gate: allow terminal cell grid, sized by the font
}
