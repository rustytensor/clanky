// A fixture for the design gate's source scan: one loose radius on line 9, and one exempted size.
use gpui::{div, px, IntoElement, Styled};

mod tokens;

pub fn card() -> impl IntoElement {
    div()
        .h(tokens::CARD_HEIGHT)
        .rounded(px(9.))
        .w(px(40.)) // gate: allow terminal cell grid, sized by the font
}
