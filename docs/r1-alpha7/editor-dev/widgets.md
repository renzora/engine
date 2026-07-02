# Custom Widgets

Build reusable UI components for editor panels and game HUDs with `renzora_ember` — plain bevy_ui builder functions, WGSL `UiMaterial` widgets, and the markup attribute kernel.

## Two ways to build a widget

Renzora's UI lives in one crate, `renzora_ember`, used by both the editor and exported games. There are two complementary ways to produce a widget:

1. **Rust builder functions** (`renzora_ember::widgets`) — a function that spawns one or more `bevy_ui` entities and returns the root `Entity`. This is how editor panels and Rust-driven HUDs are built. Registered by `WidgetsPlugin` (part of `EmberPlugin`).
2. **Markup** — author widgets declaratively in hot-reloadable `.html` files; the markup loader (`MarkupPlugin`) spawns the same `bevy_ui` entities for you, plus an attribute **interaction kernel** (`toggle=`, `drag_value=`, `fill=`, `vector=`). See *Markup & Templates* for the authoring format; this page covers the kernel attributes and how they relate to the Rust widgets.

Both paths produce ordinary `bevy_ui` entities (`Node`, `Text`, `BackgroundColor`, …). There is no retained widget VM and no per-frame re-layout pass beyond Bevy's own.

> ⚠️ **egui is gone.** `egui`/`bevy_egui` were removed from the engine entirely. There is no `egui::Ui`, no `egui::Widget` trait, no `ui.add(...)`, and no `EditorTheme`. Any example showing `impl egui::Widget` or `ui.label(...)` is a dead API — ignore it. Widgets are bevy_ui entities built from `Commands`.

## The builder-function pattern

A widget is a `pub fn` that takes `&mut Commands` (and, when it needs text, fonts), spawns the entity tree, and returns the root. The canonical signature:

```rust
use bevy::prelude::*;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{accent, border, rgb, text_primary};

/// A labelled pill that shows a status color.
pub fn status_pill(commands: &mut Commands, fonts: &EmberFonts, label: &str, ok: bool) -> Entity {
    let color = if ok { accent() } else { (200, 80, 80) };
    let pill = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("status-pill"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
        ))
        .id();
    commands.entity(pill).add_child(text);
    pill
}
```

Key conventions, matching every built-in widget:

- Colors come from the theme palette in `renzora_ember::theme`: accessor functions like `accent()`, `border()`, `card_bg()`, `window_bg()`, `text_primary()`, `text_muted()`, `tab_active()` each return an `(u8, u8, u8)` triple; `rgb(...)` turns one into a bevy `Color`. Don't hard-code hex.
- Text needs a font handle. `EmberFonts` carries three: `fonts.ui`, `fonts.phosphor` (icons), `fonts.mono`. `ui_font(&handle, size)` builds a `TextFont`; `icon_text(commands, &fonts.phosphor, name, color, size)` spawns a Phosphor glyph.
- Give every node a `Name` — it's how markup bindings, the dock, and debug tooling find it.
- For interactivity, add `Interaction::default()` and either a marker component your own system reads, or one of ember's existing markers (e.g. `EmberButton`).

To call a builder from an editor panel, use the `build` closure of `register_panel_content` (see *Building Editor Panels*), which hands you exactly `(&mut Commands, &EmberFonts)`:

```rust
app.register_panel_content("my_panel", true, |commands, fonts| {
    let root = commands.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    }).id();

    let ok  = status_pill(commands, fonts, "Connected", true);
    let bad = status_pill(commands, fonts, "Offline", false);
    commands.entity(root).add_children(&[ok, bad]);
    root
});
```

## The built-in widget library

`renzora_ember::widgets` ships ~80 widget modules, each a builder fn (or several) plus the interaction system that animates its state. They are registered by `WidgetsPlugin`. Import them with `use renzora_ember::widgets::*;`. A representative selection:

| Category | Builder fns |
|---|---|
| Buttons | `button`, `icon_button`, `icon_label_button` |
| Toggles | `checkbox`, `toggle`, `toggle_switch`, `radio`, `segmented` |
| Numeric | `slider`, `drag_value`, `spin_slider`, `stepper`, `knob`, `fader`, `range`, `xy_pad` |
| Selection | `dropdown`, `multi_select`, `search`, `tags_input` |
| Text entry | `text_input`, `textarea`, `floating_label`, `input_group`, `validation` |
| Color / curves | `color_picker`, `gradient` editor, `curve` editor |
| Data viz | `gauge`, `line_chart`, `bar_chart`, `sparkline`, `line_chart_live`, `waveform`, `vu_meter`, `mixer` |
| Containers | `card`, `section`, `accordion`, `collapsible`, `tabs`, `divider`, `scroll_area` |
| Data display | `table`, `tree`, `grid`, `avatar`, `chip`, `badge`, `list_group`, `timeline_view` |
| Overlays | `modal`, `popover`, `tooltip`, `popup`, `menu`, `context_menu`, `toast`, `alert` |
| Navigation | `navbar`, `breadcrumb`, `pagination` |
| Editors | `node_graph`, `code_editor`, `property_row`, `vec3_edit` |
| Feedback | `progress`, `spinner`, `skeleton` |

Most take `&mut Commands` plus their initial value or fonts. Examples:

```rust
let b = button(commands, &fonts.ui, "Apply");          // -> Entity
let c = checkbox(commands, true);                       // initial checked
let s = slider(commands, 0.5);                          // value in 0..1
let k = knob(commands, 0.25);
let g = gauge(commands, fonts, 0.8);                    // circular dial + % label
let chart = line_chart(commands, fonts, &samples);
let w = waveform(commands, &amplitudes);
let ng = node_graph(commands, fonts);
```

> The **Gallery** workspace in the editor is a living catalog of this widget set — open it to see every widget rendered live with its current theme.

### Tooltips (`HoverTooltip`)

Tooltips are a **global layer**, not per-widget bubbles: insert `renzora_ember::widgets::HoverTooltip::new("Label")` on any entity that has `Interaction`, and hovering it shows the shared cursor-following bubble after a short delay. Do **not** spawn a bubble node as a child of your widget — bevy_ui clips absolutely-positioned children by every scrolling/clipping ancestor, so a per-widget bubble silently disappears inside panels (`GlobalZIndex` changes paint order, not clipping). The shared bubble is a parentless root node with `Pickable::IGNORE`, so nothing clips it and it never steals hover. The `tooltip(...)` wrapper builder still exists for wrapping non-interactive content, and forwards to the same mechanism. Viewport toolbar buttons, panel toolbar buttons, and the inspector's component rail all use it.

### Code editor (`code_editor`)

The `code_editor` widget is a monospace, syntax-highlighted, editable text view. It owns no document model: the host crate attaches a `CodeBindingSpec` (via `bind_code`) of closures that shuttle text in and out — `doc_key` (document identity), `load`, `store`, `make_highlighter` (a per-language tokenizer producing colored `CodeToken` runs), and an optional `font_size` (the live zoom). `renzora_code_editor` wires this to its `CodeEditorState` (open files, active tab, zoom).

**Languages.** The tokenizer (`renzora_code_editor::highlight`) covers Lua, Rhai, Rust, WGSL, Python, Shell, SQL, JSON, TOML, **BSN** (the `.bsn` scene format — `//` / `/* */` comments, `entity`/`resource` keywords, PascalCase component type paths), and **HTML** (`.html`/`.htm` markup UI — tag names, attributes, quoted values, `&entities;`, and `<!-- -->` comments that thread across lines), picked by file extension. Cross-line state (block comments, HTML comments) threads between lines as an opaque `u32` so a comment opened off-screen still colors correctly when scrolled into view.

**Colors are themed.** Every token color and editor-chrome color comes from the active theme's `[syntax]` section via ember's `SyntaxPalette` — see *Theming → Code-editor syntax colors*. Editing them in Settings → Theme recolors the open editor live.

**Editor chrome.** Each render lays absolute-positioned overlays into the body in back-to-front order: the **current-line highlight** (`current_line`, full viewport width), **indent guides** (`indent_guide`, a vertical rule at each interior indent stop — `TAB_WIDTH` = 4 cols), the **selection** rects (`selection`), and **matching-bracket** boxes (`bracket_match`, shown when the caret is next to a bracket and has no selection; the match is found nesting-aware across lines, bounded so a huge file can't stall the render). Then the colored text rows paint on top.

**Sizing is zoom-aware.** All metrics — line height, gutter width, caret height, and the character advance — are derived from the live `font_size` (logical px) the host pushes through the binding (`CodeEditorState.font_size`, driven by Ctrl +/- and the Settings code-font size). There are no hardcoded pixel sizes.

**Advance is measured, not assumed.** Rather than hardcoding a 0.6em advance, a hidden probe (`code_probe`) reads the active mono font's real laid-out width from its `TextLayoutInfo` and feeds the per-font advance ratio back, so Fira Code / Source Code Pro / custom mono fonts get pixel-correct carets. The measurement is scale-invariant and tightly guarded — a bad/early reading falls back to 0.6 with no regression.

**Monospace is intentional.** Bevy 0.19's `PositionedGlyph` exposes a glyph's pixel position but *not* its source character/cluster index, so an arbitrary glyph can't be mapped back to a column — which is what proportional-font click/caret hit-testing would need across our multi-token text. Monospace keeps column ↔ pixel math exact and matches every real code editor; ligature mono fonts still work, since a ligature keeps the combined cell advance.

### Reactive values

Builders run once; dynamic values are wired through `renzora_ember::reactive`. A slider stores its value in a `Bound<f32>` so `bind_2way` can read and write it; text is driven with `bind_text`, visibility with `bind_display`, and variable-length lists with `keyed_list`. See *Building Editor Panels → Reactive content* for the full helper table — the same helpers drive widget contents.

### Scroll areas & remembered position

`scroll_view` / `scroll_view_bar` / `scroll_view_pinned` / `scroll_area` wrap content in a smooth-scrolling, auto-hiding-scrollbar viewport. Their position lives on the entity, so a view that gets despawned and rebuilt (a panel that re-spawns, the whole chrome rebuilding on a theme switch) normally snaps back to the top.

To keep the position across rebuilds, use the **keyed** variants and give the view a stable string key:

```rust
let s = scroll_view_keyed(commands, content, "hierarchy");          // flex-fill
let m = scroll_area_keyed(commands, content, 260.0, "status-theme-menu"); // capped
```

The offset is saved in the `ScrollMemory` resource under that key and restored — once the content is laid out — when an identically-keyed view spawns again. Use one **unique** key per logical list; two unrelated lists sharing a key would fight over the same saved offset.

**Wheel over a numeric field.** A `drag_value` (and the markup `drag_value=` kernel) only scrubs its value on **Shift+wheel**. A plain wheel is always handed to the enclosing scroll area, so dragging the panel scrollbar past a field never snags on it and silently changes the number — the panel scroll always wins, and value-scrubbing is an explicit opt-in gesture.

## Theming with `Styled` and `Role`

Instead of baking colors into a widget, attach a `Styled` component naming a `Role`. The `apply_theme` system (in `style::ThemePlugin`) repaints every `Styled` entity from the active `Theme` whenever the theme or the widget's state changes — no rebuild.

```rust
use renzora_ember::style::{Role, Styled, WidgetState};

commands.spawn((
    Node { /* … */ ..default() },
    BackgroundColor(rgb(tab_active())),   // starting color; apply_theme overrides it
    Interaction::default(),
    Styled::new(Role::Button),            // paints from theme.token(Role::Button)
    Name::new("my-button"),
));
```

`Role` values: `Button`, `ButtonAccent`, `IconButton`, `Input`, `Checkbox`, `Segment`, `Toggle`, `Card`, `Badge`, `Alert`, `Toast`, `Tab`, `Panel`, `Menu`. Each maps to a `StyleToken` with per-state fills (`bg`, `bg_hover`, `bg_pressed`, `bg_active`, `bg_disabled`), border colors, geometry (`radius`, padding), and text colors. Your interaction system sets `Styled.state` (`Normal`/`Hover`/`Pressed`/`Active`/`Disabled`); `apply_theme` does the painting:

```rust
fn my_button_interact(
    mut q: Query<(&Interaction, &mut Styled), (With<MyMarker>, Changed<Interaction>)>,
) {
    for (interaction, mut styled) in &mut q {
        styled.state = match interaction {
            Interaction::Pressed => WidgetState::Pressed,
            Interaction::Hovered => WidgetState::Hover,
            Interaction::None    => WidgetState::Normal,
        };
    }
}
```

The `Theme` is a `Reflect` + Serde resource loaded from project `themes/*.toml` (colors are `#RRGGBB` / `#RRGGBBAA` hex), so the editor and the exported game read the same theme. (Game-side UIs may instead use `renzora_game_ui::UiTheme` semantic tokens with the `UiThemed` marker.)

## GPU vector widgets — WGSL `UiMaterial`s

Gauges, charts, and waveforms aren't drawn with rectangles — they're painted by fragment shaders bound to `bevy_ui` `MaterialNode`s. ember ships three `UiMaterial`s, each backed by an embedded `.wgsl` file:

| Material | Shader | Drives |
|---|---|---|
| `ArcMaterial` | `widgets/gauge/gauge.wgsl` | `gauge`, `knob`, and markup `vector="arc"` / `speedometer` |
| `ChartMaterial` | `widgets/chart/chart.wgsl` | `line_chart`, `sparkline`, `line_chart_live`, markup `vector="line"` |
| `WaveMaterial` | `widgets/waveform/waveform.wgsl` | `waveform`, markup `vector="wave"` |

Each material plugin (`GaugePlugin`/`ChartPlugin`/`WaveformPlugin`) is `is_plugin_added`-guarded, because both `WidgetsPlugin` and the markup `vector` runtime register them and re-adding a `UiMaterialPlugin` for the same material would panic. You don't add them yourself — use the builder fns or markup.

> ⚠️ This used to be drawn with `vello` / `bevy_vello` (`UiVelloScene`, a `Camera2d` + `VelloView` on a `RenderLayers` layer). **vello was removed.** Everything now renders as ordinary `bevy_ui` `MaterialNode`s with `bevy_text` children for labels/readouts. There is likewise **no `renzora_gauges` crate** anymore — gauge drawing is ember's `gauge` widget (`ArcMaterial`).

### Vector widgets in markup

In `.html` markup, request a vector widget with the `vector=` attribute. The loader stamps a `VectorSpec` and the attach/sync systems pick the right material and bind `{{ }}` paths every frame:

```html
<!-- a dial bound to a script variable, with a centred readout -->
<node width="120px" height="120px"
      vector="gauge" value="{{ speed }}" min="0" max="240"
      color="#39d98a" readout="{{ speed }}" unit="km/h" />

<!-- a live line chart from a comma string -->
<node width="200px" height="80px" vector="line" data="{{ frame_times }}" />

<!-- a full speedometer composite: arc + ticks + numeric labels + needle -->
<node width="160px" height="160px"
      vector="speedometer" value="{{ rpm }}" min="0" max="8000"
      start="135" sweep="270" count="8" readout="{{ rpm }}" unit="rpm" />
```

Current `VectorKind`s (and their aliases):

| `vector=` | Aliases | Renders with |
|---|---|---|
| `arc` | `gauge`, `ring` | `ArcMaterial` (+ optional centred `readout`) |
| `bars` | `bar` | bevy_ui rectangles, one per datum |
| `line` | `chart` | `ChartMaterial` |
| `wave` | `waveform` | `WaveMaterial` |
| `speedometer` | `dial` | composite: `ArcMaterial` + `bevy_text` ticks/labels + needle + centre readout |

Common attributes: `value`, `data` (comma string, literal or `{{ path }}`), `min`/`max`, `color`, `track`, `fill`, `thickness`, `count`, `start` (deg, default 135), `sweep` (deg, default 270), `inset` (px), `len` (tick px), `readout`, `unit`, `size`/`readsize`.

> The standalone `ticks` / `labels` / `needle` primitives no longer exist as their own `vector=` kinds — they are assembled inside the `speedometer` composite.

## The markup interaction kernel

Separate from the Rust widget library, markup nodes can opt into behavior through kernel attributes (`renzora_ember::markup::widgets`). These are the declarative analogues of the Rust widgets, and their writes route through the scripting layer's `ScriptReflectionQueue` / `ScriptComponent`:

| Attribute | Component | Behavior |
|---|---|---|
| `toggle="Path.bool"` | `Toggle` | Click flips the bound boolean (checkbox / switch). |
| `drag_value="Path.num" drag_min drag_max` | `DragValue` | Drag horizontally to set the bound number (slider / scrollbar). |
| `fill="Path.num" fill_min fill_max` | `ValueFill` | Node width tracks the value's fraction of the range (slider fill / progress). |
| `toggles="name"` | `Disclose` | Click shows/hides the entity with that `Name` (dropdown / accordion / modal). |

Events (`on_press`, `on_enter`, `on_exit`, `on_spawn`, `on_change`) use bevy_hui's `OnUiPress`/`OnUiEnter`/`OnUiExit`/`OnUiSpawn`/`OnUiChange` components, which feed the firing node into a script's `on_ui(name, args, entity)` hook.

> There is **no `MarkupOnPress`, `MarkupId`, or `MarkupClass` component, and no `class=` attribute.** `id=` and `name=` simply set the entity's `Name`. Reference a reusable markup component with `<node template="path/to/widget.html">` — the old file-stem custom-tag registry was removed, and a bare unknown tag now warns and renders nothing.

## Writing your own GPU widget (advanced)

To paint a custom widget on the GPU, define a `UiMaterial` exactly like ember's. Embed a WGSL shader, derive `AsBindGroup`, register a `UiMaterialPlugin`, and attach a `MaterialNode` to your node. The shape mirrors `ArcMaterial`:

```rust
use bevy::asset::{embedded_asset, Asset};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct RingMaterial {
    #[uniform(0)]
    pub color: Vec4,
    #[uniform(0)]
    pub params: Vec4, // x = progress 0..1, y/z/w = your knobs
}

impl UiMaterial for RingMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://my_plugin/ring.wgsl".into()
    }
}

pub struct RingPlugin;
impl Plugin for RingPlugin {
    fn build(&self, app: &mut App) {
        // Guard against double-registration if more than one path adds it.
        if app.is_plugin_added::<UiMaterialPlugin<RingMaterial>>() {
            return;
        }
        embedded_asset!(app, "ring.wgsl");
        app.add_plugins(UiMaterialPlugin::<RingMaterial>::default());
    }
}
```

```wgsl
// ring.wgsl — a UiMaterial fragment shader.
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Ring { color: vec4<f32>, params: vec4<f32> };
@group(1) @binding(0) var<uniform> u: Ring;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let c = in.size * 0.5;
    let p = in.uv * in.size - c;
    let r = length(p) / (min(in.size.x, in.size.y) * 0.5);
    let on = step(0.85, r) * step(r, 1.0);     // a thin outer ring
    if (on <= 0.0) { discard; }
    return vec4<f32>(u.color.rgb, u.color.a * u.params.x);
}
```

Then spawn the node with a `MaterialNode`, creating the material from `Assets<RingMaterial>`:

```rust
fn spawn_ring(mut commands: Commands, mut mats: ResMut<Assets<RingMaterial>>) {
    let mat = mats.add(RingMaterial {
        color: Vec4::new(0.22, 0.85, 0.54, 1.0),
        params: Vec4::new(0.7, 0.0, 0.0, 0.0),
    });
    commands.spawn((
        Node { width: Val::Px(64.0), height: Val::Px(64.0), ..default() },
        MaterialNode(mat),
        Name::new("ring"),
    ));
}
```

Update the value by mutating the material via its handle in a system (`materials.get_mut(&node.0)`), exactly as `arc_sync`/`chart_sync` do — that's the cheapest way to animate, since it touches only the uniform buffer.

## Where widgets live

Widgets are contributed by plugins. Editor-only widgets and panels ride in editor-scope plugins (`renzora::add!(MyPlugin, Editor)`), linked into the removable `renzora_editor` bundle; game HUD widgets ride in runtime-scope plugins and ship inside the game. Either way you import builders from `renzora_ember::widgets` and theme helpers from `renzora_ember::theme` / `renzora_ember::style`. There is no `renzora::prelude` — use `use renzora::*;` or import individual items.
