"""gpui-ce's text coverage curve, so a reference glyph carries the ink a gpui-ce window gives it.

gpui-ce does not paint a glyph's coverage as it is: its monochrome sprite shader
(crates/gpui_render/src/shaders/sprites.rs:151-156) passes each sample through
apply_contrast_and_gamma_correction (crates/gpui_render/src/shaders/common.rs:341-353), with the defaults of
ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST = 1.0 and ZED_FONTS_GAMMA = 1.8
(crates/gpui_wgpu/src/wgpu_renderer/settings.rs:48-52). The glyph colour reaches the shader as sRGB components
(hsla_to_rgba, common.rs:491-495). All references are to gpui-ce ba5805cd66.
"""

GAMMA = 1.8
GRAYSCALE_ENHANCED_CONTRAST = 1.0
# common.rs:27, 39 and 40.
REC_601_LUMA_WEIGHTS = (0.30, 0.59, 0.11)
DARK_TEXT_CONTRAST_SCALE = 4.0
DARK_TEXT_BRIGHTNESS_CUTOFF = 0.75
# crates/gpui/src/platform.rs:1848-1880, the row for gamma 1.8 and the two normalisers.
GAMMA_1_8_RATIOS = (0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0)
NORM13 = 0x10000 / (255.0 * 255.0) * 4.0
NORM24 = 0x100 / 255.0 * 4.0


def gamma_ratios():
    """get_gamma_correction_ratios(1.8) (platform.rs:1848)."""
    r = GAMMA_1_8_RATIOS
    return (r[0] * NORM13, r[1] * NORM24, r[2] * NORM13, r[3] * NORM24)


def corrected_alpha(sample, colour):
    """apply_contrast_and_gamma_correction (common.rs:341) for one coverage sample in 0..1 and an sRGB colour."""
    red, green, blue = (channel / 255.0 for channel in colour)
    brightness = sum(
        w * c for w, c in zip(REC_601_LUMA_WEIGHTS, (red, green, blue))
    )  # color_brightness, :307
    darkness = min(
        max(DARK_TEXT_CONTRAST_SCALE * (DARK_TEXT_BRIGHTNESS_CUTOFF - brightness), 0.0),
        1.0,
    )
    contrast = GRAYSCALE_ENHANCED_CONTRAST * darkness  # light_on_dark_contrast, :310
    alpha = (
        sample * (contrast + 1.0) / (sample * contrast + 1.0)
    )  # enhance_contrast, :317
    x, y, z, w = gamma_ratios()
    correction = (brightness * x + y) * alpha + (
        brightness * z + w
    )  # apply_alpha_correction, :327
    return alpha + alpha * (1.0 - alpha) * correction


def corrected_mask(mask, colour):
    """A Pillow L mask of coverage, passed through the curve for glyphs drawn in COLOUR."""
    table = [
        min(max(round(corrected_alpha(value / 255.0, colour) * 255.0), 0), 255)
        for value in range(256)
    ]
    return mask.point(table)
