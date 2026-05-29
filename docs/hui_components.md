# renzora_hui — component catalog & roadmap

The goal: a toolbox to build any UI. The trick is that **most widgets are
markup compositions** over a handful of engine-side *behaviors*. Build the
behavior kernel once; everything else is `.html`.

Legend: ✅ have · 🟡 markup-only (buildable today, no Rust) · 🔧 needs a
behavior kernel piece · ⬜ not started

---

## 1. Primitives (✅ have)
- `<node>` — the flex/grid box. Layout, color, border, radius, padding…
- `<text>` — text + font size/color. Supports `{{ binding }}`.
- `<image>` — textures (`src=`, drag-drop in inspector).
- `<button>` — `on_press` / `on_enter` / `on_exit` → script `on_ui`.

## 2. Composition & control flow (✅ have)
- `<node template="path.html">` — reusable component, props cascade, `<slot/>`.
- `<for tag="...">` — repeat per matching entity.
- `show="{{ cond }}"` — conditional (and/or/not, comparisons).
- `{{ Component.field }}` / `{{ Entity.Component.field }}` / `{{ _scriptVar }}`
  / `{{ Name }}` — reactive bindings (read).

## 3. Behavior kernel (🔧 — build these once, reuse everywhere)
These are the only truly new engine systems needed. Each is a markup attribute
or marker + a small system.

| Kernel | Markup | Powers |
|---|---|---|
| **Focus** | `focusable`, click-to-focus, Tab order | input, dropdown, anything keyboard |
| **Text input** | `<input bind="...">` | input, textarea, search, password, number, chat |
| **Drag-value** | `<node drag_value="..." min max>` | slider, scrollbar, progress-drag, color picker |
| **Toggle** | `toggle="Path.field"` | checkbox, switch, radio |
| **Disclosure** | `toggles="#id"` (show/hide a target) | dropdown, accordion, modal, tooltip, popover, tabs |
| **Two-way write** | `bind="Entity.Component.field"` | the write path all inputs share |

Build order: **Focus + Text input first** (unlocks forms/login), then
Toggle, then Drag-value, then Disclosure.

## 4. Widget catalog (🟡 markup once the kernel exists; some 🟡 today)

### Inputs
- Button ✅
- Text input ⬜ (text-input kernel)
- Password field ⬜ (input + `password="true"`)
- Number / stepper ⬜ (input + validation, or drag-value)
- Textarea / multiline ⬜ (text-input kernel)
- Checkbox ⬜ (toggle kernel)
- Switch / toggle ⬜ (toggle kernel)
- Radio group ⬜ (toggle + `for`)
- Slider ⬜ (drag-value kernel)
- Range / dual slider ⬜ (drag-value ×2)
- Dropdown / select ⬜ (disclosure + `for` + toggle)
- Combo box (filterable) ⬜ (input + dropdown)
- Color picker ⬜ (drag-value + sliders)
- Date/time ⬜ (later)

### Containers / layout
- Panel / card / chip / kbd 🟡 (have as templates)
- Stack / row / grid 🟡 (`<node flex/grid>`)
- Scroll area ⬜ (overflow + drag-value scrollbar)
- Tabs ⬜ (disclosure + buttons)
- Accordion / collapsible ⬜ (disclosure)
- Split / resizable pane ⬜ (drag-value)
- Divider 🟡

### Display / feedback
- Progress bar 🟡 (have: `<progress>`)
- Spinner / loading ⬜ (needs a rotate animation — transitions feature)
- Avatar / badge 🟡
- Tooltip ⬜ (disclosure on hover)
- Toast / notification 🟡 (have template; auto-dismiss needs a timer)
- Tag / pill 🟡 (have: `<chip>`)
- Skeleton / shimmer ⬜ (transitions)
- Icon ⬜ (icon font / `<image>`)

### Navigation / overlays
- Menu bar / dropdown menu ⬜ (disclosure)
- Context menu ⬜ (disclosure + right-click)
- Modal / dialog 🟡 (have templates; focus-trap needs focus kernel)
- Drawer / sidebar 🟡 (+ transition for slide)
- Breadcrumb 🟡
- Pagination 🟡 (`for` + buttons)

### Data
- List 🟡 (`<for>`) / virtualized list ⬜ (later)
- Table ⬜ (`for` + grid; sortable needs script)
- Tree view ⬜ (recursive template + disclosure)

## 5. Polish layer (⬜ later)
- **Transitions/tweens** — smooth hover/show, slide-in drawers, spinners.
  bevy_hui already parses `hover:` / `delay=` / `ease=`; needs a tween system.
- **Theme tokens** — project `theme.html` with `{accent}` etc.; one file
  re-themes everything.
- **`json_stringify`** — symmetric with `json_parse` for clean request bodies.

---

## The point
Sections 1–2 are done. Section 3 is ~5 small systems. Once the kernel exists,
the entire Section 4 catalog is authored as `.html` files in a `widgets/`
folder — no further Rust. That's how you "build anything": a small behavior
kernel + a growing markup widget library.
