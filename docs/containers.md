# Containers (Panel, GroupBox): evaluation

**Not implemented.** Containers are not a small change: every control is created on a `Host` that is the window,
and the designer assumes a flat list of controls on one surface. This note records what a container needs, where the
cost is, and the order to do it in.

## Why it is not small

A control is created through `Button::new(host, ...)`, where `host: &dyn Host` and, today, the only `Host`s are the
window itself. A container needs to *be* a `Host` too, so children can be created inside it and positioned relative
to it. That part is easy. The cost is everything that assumes the window is the only parent.

## Recommended approach

Make a container just another `Host`:

```rust
pub trait PanelHandle: WidgetHandle + Host {}
pub struct Panel { handle: Box<dyn PanelHandle> }
impl Host for Panel { /* forwards create_* to the handle */ }
// Button::new(&panel, "OK") then works unchanged.
```

`GroupBox` is a `Panel` with a caption (`text`) and a border.

### Backends

| Backend | Work | Rough size |
|---|---|---|
| web (wasm-bindgen) | A `<div>` (`<fieldset>` for a group box) whose children are appended to it. The DOM already nests; coordinates are relative to the container for free. | small |
| web-min | `rad_create_element` already takes a `parent_id`; the runtime looks up `elements[parentId].body`, so a container element only needs a `body` that is its own element. | small |
| Win32 | `create_child(parent, ...)` already takes any `HWND`. The catch: `WM_COMMAND`, `WM_HSCROLL` and `WM_CTLCOLOR*` go to a control's *parent*, which would now be the panel, not the window. The panel needs its own window class whose procedure forwards those messages (or the shared procedure is reused for it). `Win32Host::from_hwnd` shows the shape of a `Host` over a bare `HWND`. | medium |

### Designer (the expensive part)

- **Drop targets.** A drop must find the innermost container under the cursor and convert the position to that
  container's coordinates. Today one `drop` listener on the form body places everything.
- **A tree, not a list.** `controls: Vec<PlacedControl>` needs a parent link (or a tree). Selection, the
  properties panel, overlays and handler wiring all index that flat list.
- **Generated code order.** Parents must be created before their children, and a child is created with
  `Button::new(&panel1, ...)`; names stay unique across the whole form. `Form1Controls` stays flat.
- **Selection overlay and dragging.** Each container needs its own overlay layer, and dragging a control must be
  able to re-parent it (drop onto a different container), with the coordinate change that implies.
- **Design-time chrome.** A container must be selectable by its border/caption without swallowing clicks meant for
  its children.

## Cost and order

Roughly: core and the three backends are about a day of work and can be tested from code alone (extend the
all-controls demo with a panel); the designer nesting is the larger piece, about one to two days, and touches most of
`rad-designer/src/lib.rs`. Do the first part first: it is independently useful (hand-written forms get containers) and
de-risks the Win32 message routing before the designer work starts.
