# The control record

`clanky-gpui` writes this record when it starts with the environment variable `CLANKY_CONTROL_RECORD=<path>`: one JSON
object per line, appended, written only when something changed. `gate-controls.py` and `gate-design-tokens.py` read
it; the G2 window code is built to this file. Without the variable nothing is written.

Units: every box is `[x, y, w, h]` in **device pixels**, whole numbers, measured from the window's top-left corner.
A box is the control's painted outer box including its border; never the box of an inner element. The `hit` box is
the area the pointer really hits, taken from the control's `Hitbox` and converted to device pixels the way painting
converts bounds, never copied from `box`.

## The rule for the window code

`hit` is only worth recording if it is the box that takes the clicks. The control's interactive element (the one with
its click, hover and press handlers) is the element the measuring child lies over, and it fills the wrapper: no margin
or padding around it inside the wrapper. gpui does not let one element read another's hitbox, so the record's `hit` is
the measuring child's own hitbox over the same bounds; handlers on some other, larger or smaller element would make it
a copy of `box` in effect. The gates' pointer probes one device pixel inside and outside each edge test the real
control and are the check on this rule.

## Screen line

Written whenever a screen's controls, focus or keyboard flag change:

```
{"record":"screen","screen":NAME,"scale":S,"focus":ID|null,"keyboard":true|false,
 "popover":null|{"box","opener","opener_id","opener_pane"},"controls":[CONTROL,...]}
```

`screen` is `main-window` or `context-menu` in G2 (G3 and G4 add the others). `keyboard` is true when the focus came
from the keyboard (so the focus ring is drawn). A screen with no controls writes its line with an empty list. `popover`
is null unless a popover is open; `box` is its outer box, `opener` the box of what opened it (a control's box, or a
zero-size box at the pointer for a right-click on a pane), `opener_id` that control's id, `opener_pane` the pane's id
for a right-click on a pane.

## Control

| Field | Meaning | gpui-ce source (our copy, branch `clanky`, head 2c9b682a7a) |
|---|---|---|
| `id` | number, stable from frame to frame | the app's own |
| `kind` | `button-small`, `button-bar`, `button`, `button-large`, `icon-button`, `chip`, `tab`, `switch`, `field`, `menu-item`, `list-row`, `palette-row`, `segment` | the app's own |
| `label` | text, empty when none | the app's own |
| `box` | painted outer box | bounds of an element laid out without a border of its own around the control: `canvas` prepaint bounds (`elements/canvas.rs:10`), already snapped by `LayoutEngine::layout_bounds` (`taffy.rs:630`); multiply by `Window::scale_factor` (`window.rs:3001`) |
| `hit` | box the pointer hits | `Hitbox.bounds` (`window.rs:971`) from `Window::insert_hitbox` (`window.rs:5558`), rounded to device pixels as `Hitbox::contains_at_scale` does (`window.rs:995`) |
| `state` | `rest`, `hover`, `pressed`, `focus` or `disabled` | disabled: the app; pressed: mouse-down listener on the control (`elements/div.rs:142`, `:226`) while the pointer is over it; focus: `FocusHandle::is_focused` (`window.rs:521`) together with `keyboard`; hover: `Hitbox::is_hovered` (`window.rs:1029`) |
| `enabled` | false for a disabled control | the app |
| `tab` | true when Tab reaches the control (equal to `enabled`) | `FocusHandle::tab_stop` (`elements/div.rs:980`); order from `tab_stop.rs` |
| `group` | name of the group the arrow keys move within, or null | the app |
| `axis` | `vertical` or `horizontal` when `group` is set, else null | the app |
| `checked` | tab: active; switch: on; segment: selected; else null | the app |
| `primary` | true for the primary button of a dialog | the app |
| `label_box` | box of the label's text, or null | a measuring child, as `control`'s box is |
| `icon` | box of the icon, or null | a measuring child |

## Other lines

- Activation: `{"record":"activate","screen":NAME,"id":ID,"label":TEXT,"via":"release"|"key"}`, written each time a
  control fires, from its `on_click` (`elements/div.rs:677`) with the click's kind (`interactive.rs:290`): a mouse click
  is `release`, a keyboard click is `key`. Nothing is written on press.
- Landmark: `{"record":"landmark","name":NAME,"index":N,"box":[x,y,w,h]}`, written when it changes (`index` is 0 unless a name repeats), for the parts the design
  gate measures that are not controls. Names: `title-bar` (the whole bar), `status-line` (the whole line, its top line
  included), `pane` (one line per pane, `index` counting from 0 in reading order, the pane's outer box), `status-count` (the box of the single digit that counts
  tabs in the status line, on its own text element), `meta-text` (the block meta row's text; the gate's meta cap-height check needs a finished block on screen and fails when
  the landmark is absent), `pane-text` (the first row of a pane's text, for the selection drag).
- Frame: `{"record":"frame","n":N}` is allowed and ignored.

## Checked by the gates

`whole-pixels` fails when a box has a fractional number. `hit-box` fails when `hit` differs from `box`, or is smaller
than 24 logical px either way. A record line that is not valid JSON, a control with a `kind` outside the list, or a
missing field is a failure of the record, named in the gate's output.
