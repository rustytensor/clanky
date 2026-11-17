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
for a right-click on a pane. Each floating screen has its own name (`palette`, `context-menu`, `agent-picker`,
`completion-list`, `find-bar`; the debug build's `test-popover` and `test-popover-corner`), and its line carries the
popover object while it is open.

## Control

| Field | Meaning | gpui-ce source (our copy, branch `clanky`, head 2c9b682a7a) |
|---|---|---|
| `id` | number, stable from frame to frame | the app's own |
| `kind` | `button-small`, `button-bar`, `button`, `button-large`, `icon-button`, `chip`, `tab`, `switch`, `field`, `menu-item`, `list-row`, `palette-row`, `segment`, `slider` (G4) | the app's own |
| `label` | text, empty when none | the app's own |
| `box` | painted outer box | bounds of an element laid out without a border of its own around the control: `canvas` prepaint bounds (`elements/canvas.rs:10`), already snapped by `LayoutEngine::layout_bounds` (`taffy.rs:630`); multiply by `Window::scale_factor` (`window.rs:3001`) |
| `hit` | box the pointer hits | `Hitbox.bounds` (`window.rs:971`) from `Window::insert_hitbox` (`window.rs:5558`), rounded to device pixels as `Hitbox::contains_at_scale` does (`window.rs:995`) |
| `state` | `rest`, `hover`, `pressed`, `focus` or `disabled` | disabled: the app; pressed: mouse-down listener on the control (`elements/div.rs:142`, `:226`) while the pointer is over it; focus: `FocusHandle::is_focused` (`window.rs:521`) together with `keyboard`; hover: `Hitbox::is_hovered` (`window.rs:1029`) |
| `enabled` | false for a disabled control | the app |
| `tab` | true when Tab reaches the control (equal to `enabled`) | `FocusHandle::tab_stop` (`elements/div.rs:980`); order from `tab_stop.rs` |
| `group` | name of the group the arrow keys move within, or null | the app |
| `axis` | `vertical` or `horizontal` when `group` is set, else null | the app |
| `checked` | tab: active; switch: on; segment: selected; menu item or list row: the selected row of a list whose keyboard stays in its field or popover (drawn in `accent`); else null | the app |
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

## G4 contract: the Settings window

The Settings window writes the screen `settings` (one line per change, like every screen) while it is open. Its
section switcher is a group of `segment` controls (group `settings-pages`, axis `horizontal`, `checked` on the open
page); the open page's rows follow as controls. Everything below is new for G4.

- **Screen line**: `settings` adds `"page": NAME` (`General`, `Appearance`, `Terminal`, `Editor`, `Agents`, `History`
  or `Keys`) and `"scroll": {"top": N, "max": M}` (device px the body is scrolled, and the most it can scroll).
  `popover` is null; Settings is a window of its own over the main window.
- **A control per setting**, on a card with the setting's title and description:
  `switch` (`checked`), `segment` per option (group `row:KEY`, axis `horizontal`, `checked` on the chosen one, `label`
  the option's text), `field` and `slider`. The control's `label` is the setting's title (a segment's is its option),
  so a gate finds a row by its words; the row's config key is not in the record, the gate has it from `rows.json`.
- **`field_text`** (kind `field` only): the text in the field now, as typed, before it is committed. Enter commits,
  Escape puts the saved text back.
- **kind `slider`** (Opacity): `value`, `min`, `max` (numbers) next to the usual fields. `box` is the whole control; a
  click at a fraction of its width sets `min + fraction x (max - min)`, arrows move it by a step.
- **`visible`** (every control of the `settings` screen): true when the control's whole box is inside the body (below
  the header, above the panel's bottom edge). A control scrolled out still has its line, with the box it would have,
  and `visible: false`; Tab to it scrolls it into view. A gate scrolls with the wheel (X buttons 4 and 5) and reads again.
- **`error-banner` landmark**: `{"record":"landmark","name":"error-banner","index":0,"box":[x,y,w,h],"text":TEXT}`;
  the box is `[0,0,0,0]` and `text` empty when no banner is shown. `text` is the words on screen, such as
  `config.xml line 4: unexpected end of element`. Landmarks may carry `text` from now on.
- **Landmarks for the design gate**: `settings-panel` (outer box of the Settings window), `settings-header` (the whole
  header row including its bottom divider), `settings-tab-pill` (the section switcher's fill), `settings-group-label`
  (the label's text line box, `index` in reading order), `settings-card` (outer box of each card, with its border,
  `index` in reading order), `settings-card-title` and `settings-card-description` (the text line boxes of card
  `index`). Cards scrolled out of the body keep their landmark with the box they would have.
- **Checked by the gates**: `settings-*` rows of `gate-design-tokens.py`, every page in `gate-controls.py`, and the
  config round trip `config-roundtrip` (`gate-controls.py`, `scripts/settings/rows.json`).
