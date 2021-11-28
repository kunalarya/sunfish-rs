# Helper to auto-generate the GUI RON file.

from typing import Union, TextIO, Tuple

import attr


@attr.frozen
class ScreenMetrics:
    width: int
    height: int


# Standalone, big:
# TODO: Extract Sprite width & height from PNG directly.
SPRITE_WIDTH, SPRITE_HEIGHT = 1500, 997

UI_SCALE = 1.0
WIDTH, HEIGHT = int(SPRITE_WIDTH * UI_SCALE), int(SPRITE_HEIGHT * UI_SCALE)
RATIO = HEIGHT / WIDTH

METRICS = ScreenMetrics(width=WIDTH, height=HEIGHT)

DEFAULT_PADDING_X = 0.005
DEFAULT_PADDING_Y = 0.001

# Spinners
SP_VALUE_SCALE = 0.016
SP_VALUE_COLOR = "Color(r: 0.30039, g: 0.30039, b: 0.3019)"
# SP_VALUE_COLOR_LIGHT = "Color(r: 1.0, g: 1.0, b: 1.0)"
KNOB_FONT_SCALE = 0.016

# Knobs
KN_LIGHT_NOTCH_COLOR = "Color(r: 0.0429, g: 0.0468, b: 0.0507)"
KN_STD_ARC_COLOR = "Color(r: 0.7, g: 0.7, b: 0.7)"
KN_VALUE_FONT_SCALE = 0.013
KN_VALUE_COLOR = "Color(r: 0.30039, g: 0.30039, b: 0.3019)"
KN_DEF_VALUE_POS = "Below(offset_relative: Some(0.006))"

# Sliders
SL_DEF_VALUE_POS = "Below(offset_relative: Some(0.019))"

BACKGROUND_UNSCALED = (40, 50, 64)

BACKGROUND_COLOR = (float(c) / 256.0 for c in BACKGROUND_UNSCALED)

Padding = Union[float, Tuple[float, float]]


NumLike = Union[float, int]


@attr.frozen
class Rect:
    x1: NumLike
    y1: NumLike
    x2: NumLike
    y2: NumLike

    def to_str(self) -> str:
        return (
            f"Rect(pos: ({self.x1:.6f}, {self.y1:.6f}, {self.x2:.6f}, {self.y2:.6f}))"
        )

    def normalized(self, screen: ScreenMetrics) -> str:
        ratio = SPRITE_HEIGHT / SPRITE_WIDTH
        nx1 = self.x1 / screen.width
        nx2 = self.x2 / screen.width
        ny1 = (self.y1 * ratio) / screen.height
        ny2 = (self.y2 * ratio) / screen.height
        return f"Rect(pos: ({nx1:.6f}, {ny1:.6f}, {nx2:.6f}, {ny2:.6f}))"

    @classmethod
    def from_offset(
        cls, x: NumLike, y: NumLike, width: NumLike, height: NumLike
    ) -> "Rect":
        return Rect(x, y, x + width, y + height)


def create_osc_panel(screen: ScreenMetrics, out: TextIO, osc: int) -> None:
    x_offset = 740 * (osc - 1)
    on_off_rect = Rect.from_offset(x_offset + 50, 35, 43, 31)
    shape_rect = Rect.from_offset(x_offset + 144, 136, 201, 33)
    octave_rect = Rect.from_offset(x_offset + 149, 230, 79, 33)
    semi_rect = Rect.from_offset(x_offset + 276, 230, 69, 33)
    fine_rect = Rect.from_offset(x_offset + 384, 230, 69, 33)
    stereo_rect = Rect.from_offset(x_offset + 538, 262, 59, 59)
    gain_rect = Rect.from_offset(x_offset + 628, 262, 59, 59)
    unison_voices_rect = Rect.from_offset(x_offset + 529, 141, 86, 27)
    unison_amt_rect = Rect.from_offset(x_offset + 637, 138, 38, 38)

    if osc == 1:
        toggle_sprite_on = Rect.from_offset(0, 997.5, 42.5, 29.5)
    else:
        toggle_sprite_on = Rect.from_offset(43, 997.5, 42.5, 29.5)
    toggle_sprite_off = Rect.from_offset(x_offset + 50, 35, 43, 31)

    def emit(rect: Rect) -> str:
        return rect.normalized(screen)

    # Panel
    out.write(
        f"""
        // // OSC {osc} Panel
        Toggle(
            widget_id: Bound(eparam: Osc{osc}(Enable)),
            rect: {emit(on_off_rect)},
            label: None,
            sprite: Some(ToggleSprite(on: {toggle_sprite_on.to_str()}, off: {toggle_sprite_off.to_str()}))
        ),
        Spinner(  // Shape
            widget_id: Bound(eparam: Osc{osc}(Shape)),
            rect: {emit(shape_rect)},
            label: None, 
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Spinner(  // Octave
            widget_id: Bound(eparam: Osc{osc}(OctaveOffset)),
            rect: {emit(octave_rect)},
            label: None, 
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Spinner(  // Semi
            widget_id: Bound(eparam: Osc{osc}(SemitonesOffset)),
            rect: {emit(semi_rect)},
            label: None, 
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Spinner(
            widget_id: Bound(eparam: Osc{osc}(FineOffset)),
            rect: {emit(fine_rect)},
            label: None,
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Osc{osc}(StereoWidth)),
            rect: {emit(stereo_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
            polarity: Some(Bipolar),
        ),
        Knob(
            widget_id: Bound(eparam: Osc{osc}(Gain)),
            rect: {emit(gain_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
        Spinner(
            widget_id: Bound(eparam: Osc{osc}(Unison)),
            rect: {emit(unison_voices_rect)},
            label: None, 
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Osc{osc}(UnisonAmt)),
            rect: {emit(unison_amt_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
    """
    )


def create_filt_panel(screen: ScreenMetrics, out: TextIO, filt: int) -> None:
    x_offset = 740 * (filt - 1)
    on_off_rect = Rect.from_offset(x_offset + 50, 423, 43, 31)
    mode_rect = Rect.from_offset(x_offset + 135, 537, 201, 33)
    cutoff_rect = Rect.from_offset(x_offset + 526, 484, 59, 59)
    res_rect = Rect.from_offset(x_offset + 628, 484, 59, 59)
    env_rect = Rect.from_offset(x_offset + 590, 593, 39, 39)

    if filt == 1:
        toggle_sprite_on = Rect.from_offset(86, 997.5, 42.5, 29.5)
    else:
        toggle_sprite_on = Rect.from_offset(129, 997.5, 42.5, 29.5)
    toggle_sprite_off = Rect.from_offset(x_offset + 50, 423, 43, 31)

    def emit(rect: Rect) -> str:
        return rect.normalized(screen)

    out.write(
        f"""
        // Filter {filt} Panel
        Toggle(
            widget_id: Bound(eparam: Filt{filt}(Enable)),
            rect: {emit(on_off_rect)},
            label: None,
            sprite: Some(ToggleSprite(on: {toggle_sprite_on.to_str()}, off: {toggle_sprite_off.to_str()}))
        ),
        Spinner(
            widget_id: Bound(eparam: Filt{filt}(Mode)),
            rect: {emit(mode_rect)},
            label: None,
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Filt{filt}(Cutoff)),
            rect: {emit(cutoff_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Filt{filt}(Resonance)),
            rect: {emit(res_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Filt{filt}(EnvAmt)),
            rect: {emit(env_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
    """
    )


def create_lfo_panel(screen: ScreenMetrics, out: TextIO, lfo: int) -> None:
    x_offset = 373 * (lfo - 1)
    target_rect = Rect.from_offset(x_offset + 103, 788, 220, 33)
    shape_rect = Rect.from_offset(x_offset + 103, 827, 220, 33)
    rate_rect = Rect.from_offset(x_offset + 118, 900, 45, 45)
    amt_rect = Rect.from_offset(x_offset + 186, 900, 45, 45)

    def emit(rect: Rect) -> str:
        return rect.normalized(screen)

    out.write(
        f"""
        // LFO{lfo}
        // TODO: Button for Synced
        Spinner(
            widget_id: Bound(eparam: Lfo{lfo}(Target)),
            rect: {emit(target_rect)},
            label: None,
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Spinner(
            widget_id: Bound(eparam: Lfo{lfo}(Shape)),
            rect: {emit(shape_rect)},
            label: None,
            value_text: Text(pos: Middle, value: "", scale: {SP_VALUE_SCALE}),
            value_text_color: {SP_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Lfo{lfo}(Rate)),
            rect: {emit(rate_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
        Knob(
            widget_id: Bound(eparam: Lfo{lfo}(Amt)),
            rect: {emit(amt_rect)},
            arc_color: {KN_STD_ARC_COLOR},
            notch_color: {KN_LIGHT_NOTCH_COLOR},
            label: None,
            value_text: Text(pos: {KN_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
            value_text_color: {KN_VALUE_COLOR},
        ),
    """
    )


def create_adsr_panel(screen: ScreenMetrics, out: TextIO, adsr: int) -> None:
    x_offset = 369 * (adsr - 1)
    attack_rect = Rect.from_offset(x_offset + 818, 790, 32, 123)
    decay_rect = Rect.from_offset(x_offset + 876, 790, 32, 123)
    sustain_rect = Rect.from_offset(x_offset + 934, 790, 32, 123)
    release_rect = Rect.from_offset(x_offset + 992, 790, 32, 123)

    thumb_sprite = Rect.from_offset(172, 997, 32, 26)

    name = "Mod" if adsr == 1 else "Amp"

    def emit(rect: Rect) -> str:
        return rect.normalized(screen)

    # Panel
    out.write(
        f"""
            // ADSR {name} Panel
            VSlider(
                widget_id: Bound(eparam: {name}Env(Attack)),
                rect: {emit(attack_rect)},
                sprite: Some(VSliderSprite(active: {thumb_sprite.to_str()})),
                value_text: Text(pos: {SL_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
                value_text_color: {KN_VALUE_COLOR},
            ),
            VSlider(
                widget_id: Bound(eparam: {name}Env(Decay)),
                rect: {emit(decay_rect)},
                sprite: Some(VSliderSprite(active: {thumb_sprite.to_str()})),
                value_text: Text(pos: {SL_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
                value_text_color: {KN_VALUE_COLOR},
            ),
            VSlider(
                widget_id: Bound(eparam: {name}Env(Sustain)),
                rect: {emit(sustain_rect)},
                sprite: Some(VSliderSprite(active: {thumb_sprite.to_str()})),
                value_text: Text(pos: {SL_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
                value_text_color: {KN_VALUE_COLOR},
            ),
            VSlider(
                widget_id: Bound(eparam: {name}Env(Release)),
                rect: {emit(release_rect)},
                sprite: Some(VSliderSprite(active: {thumb_sprite.to_str()})),
                value_text: Text(pos: {SL_DEF_VALUE_POS}, value: "", scale: {KN_VALUE_FONT_SCALE}),
                value_text_color: {KN_VALUE_COLOR},
            ),
            """
    )


def main() -> None:
    screen = METRICS

    with open("output.ron", "w") as file:
        file.write("(\n")
        file.write("""stylesheet_image: Some("synth4_background.png"),\n""")
        file.write(f"size: ({WIDTH}, {HEIGHT}),\n")
        file.write(f"padding: ({DEFAULT_PADDING_X}, {DEFAULT_PADDING_Y}),\n")

        r, g, b = BACKGROUND_COLOR
        # file.write(f"background: Solid(color: Color(r: {r}, g: {g}, b: {b})),\n")

        # Normalize sprite height & width
        if SPRITE_HEIGHT > SPRITE_WIDTH:
            normalized_height = 1.0
            normalized_width = SPRITE_WIDTH / SPRITE_HEIGHT
        else:
            normalized_height = SPRITE_HEIGHT / SPRITE_WIDTH
            normalized_width = 1.0

        bg_dst = Rect(0, 0, normalized_width, normalized_height)
        bg_src = Rect(0, 0, SPRITE_WIDTH, SPRITE_HEIGHT)
        file.write(
            f"background: Sprite(dest_rect: {bg_dst.to_str()}, src_rect: {bg_src.to_str()}),\n"
        )

        file.write(" elements: [\n")
        create_osc_panel(screen, file, 1)
        create_osc_panel(screen, file, 2)
        create_filt_panel(screen, file, 1)
        create_filt_panel(screen, file, 2)
        create_adsr_panel(screen, file, 1)
        create_adsr_panel(screen, file, 2)
        create_lfo_panel(screen, file, 1)
        create_lfo_panel(screen, file, 2)
        file.write("])\n")


if __name__ == "__main__":
    main()
