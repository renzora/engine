# renzora_hui — template (markup) API

Author UI as `.html` markup that compiles to a real `bevy_ui` entity tree.
This is the reference for elements, attributes, bindings, and control flow.

> What renders it: a template needs a **`UiCanvas`** as its UI root. Add an
> `HtmlTemplate` component (path to a `.html`) to a `UiCanvas` entity, or spawn
> from script with `action("hui_spawn", { template = "templates/x.html" })`,
> or drag a `.html` onto the viewport.

---

## File shape

```html
<template>
    <node ...>
        ...one root element, with any nesting...
    </node>
</template>
```

A **component** template also declares properties and a slot:

```html
<template>
    <property name="label">Default</property>
    <property name="color">#4C8BF5</property>
    <node background="{color}">
        <text>{label}</text>
        <slot/>            <!-- caller's children land here -->
    </node>
</template>
```

Use a component by path:
```html
<node template="templates/components/chip.html" label="OK" color="#2ECC71" />
```

---

## Elements
- `<node>` — the box (flex/grid container). The workhorse.
- `<text>` — text content (supports `{{ bindings }}`).
- `<image>` — texture via `src`.
- `<button>` — like `<node>` but emits interaction events.
- `<slot/>` — in a component, where the caller's children are placed.
- `<for tag="...">` — repeat children per matching entity (see Control flow).
- `<node template="path.html">` — expand another template here.

---

## Values & units
- Lengths: `10px`, `50%`, `10vw`, `10vh`, `5vmin`, `5vmax`, `auto`
- Grid tracks: `(count,size)` e.g. `grid_template_columns="(4,1fr)"` — **no spaces**
- Colors: `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb(255,0,0)`, `rgba(255,0,0,0.5)`
  — **no spaces inside `rgb()/rgba()`**. (`transparent` is not a keyword — use
  `#00000000`.)
- Multi-value (padding/margin/border): `"8px"` (all), `"8px 12px"` (v h),
  `"4px 8px 4px 8px"` (t r b l)

---

## Layout attributes (applied)
**Box & position**
`position` (`absolute`/`relative`) · `left` `right` `top` `bottom` ·
`width` `height` · `min_width` `min_height` `max_width` `max_height` ·
`aspect_ratio` · `padding` `margin` · `border` `border_color` `border_radius` ·
`background`

**Flex**
`display` (`flex`/`grid`/`block`/`none`) · `flex_direction`
(`row`/`column`/`row_reverse`/`column_reverse`) · `flex_wrap` · `flex_grow`
`flex_shrink` `flex_basis` · `row_gap` `column_gap` · `justify_content`
`align_items` `align_content` `justify_items` `align_self` `justify_self`

**Grid**
`grid_template_rows` `grid_template_columns` · `grid_auto_flow`
`grid_auto_rows` `grid_auto_columns` · `grid_row` `grid_column`

**Text** (`<text>`)
`font_size` · `font_color` · text content goes between the tags

**Image** (`<image>`)
`src` (asset path)

---

## Identity & behavior attributes (applied)
- `name="..."` — sets the entity's Bevy `Name`. Used by scripts (`set_on`),
  by bindings (`{{ Name }}`, `{{ ThatName.Component.field }}`), and by special
  systems. **`name="cursor_follow"` makes the entity track the mouse** (custom
  cursor; OS cursor auto-hides).
- `draggable="true"` — the node follows the mouse while dragged (LMB).
- `template="path.html"` — expand a component template onto this element;
  unknown attributes + `src` cascade as its `{prop}` overrides.
- `show="{{ cond }}"` — conditional visibility (see Control flow).

## Events (applied → scripts)
`on_press` `on_enter` `on_exit` `on_spawn` `on_change` — each names a callback
delivered to every script's `on_ui(name, args, entity)` hook.
```html
<button on_press="start_game" on_enter="hover_play">Play</button>
```

---

## Bindings — `{{ }}` (reactive, read)
Re-evaluated every frame against live ECS / script state.

| Form | Reads |
|---|---|
| `{{ Component.field }}` | the **host** entity's component (the one with `HtmlTemplate`); walks up parents |
| `{{ Component.field.sub }}` | nested fields (`Sun.color.x`) |
| `{{ EntityName.Component.field }}` | a **named** entity's component |
| `{{ _scriptVar }}` / `{{ EntityName._scriptVar }}` | a Lua `props()` variable |
| `{{ Name }}` / `{{ EntityName.Name }}` | an entity's `Name` |

Works in text content **and** attribute values:
```html
<text>HP: {{ Player.Health.current }}/{{ Player.Health.max }}</text>
<text>{{ Name }}</text>
```
> `{single_brace}` is **build-time** property substitution (component props),
> evaluated once. `{{ double_brace }}` is **runtime** reactive binding.

---

## Control flow

### Conditional — `show`
```html
<node show="{{ Player.Health.current < 20 }}">LOW</node>
<node show="{{ Sun.elevation > 0 and Sun.shadows_enabled }}">Day w/ shadows</node>
<node show="{{ not paused }}">running</node>
<node show="{{ (t < 9 or t > 17) and Sun.elevation > 0 }}">golden hour</node>
```
- Operators: `<` `>` `<=` `>=` `==` `!=`, `and`, `or`, `not` (`!`), `( )`
- Operands: binding paths, numbers, `true`/`false`, `"quoted strings"`
- Truthy fallback: non-empty / non-zero / non-`false` is true
- False → `display:none` (removed from layout; siblings reflow)

### Loop — `<for>`
```html
<for tag="enemy" flex_direction="column" row_gap="6px">
    <node padding="8px">
        <text>{{ Name }} — {{ Health.current }}/{{ Health.max }}</text>
    </node>
</for>
```
- Repeats its body once per entity carrying `EntityTag{ tag: "enemy" }`.
- Inside the body, bare bindings (`{{ Health.current }}`, `{{ Name }}`) read
  the **matched entity**.
- The `<for>` is itself a styled flex container.
- One root element per item keeps rows clean.

---

## Parsed but NOT yet applied by renzora_hui
bevy_hui's parser accepts these, but the renzora loader doesn't act on them
yet (planned). They won't error — they're just ignored:
- **Transitions**: `hover:` / `pressed:` / `active:` prefixes, `delay`,
  `duration`, `ease`, `direction`, `iterations`
- **Sprite animation**: `atlas`, `fps`, `frames`, `image_region`, `image_mode`
- **Effects**: `shadow_color`/`shadow_blur`/`shadow_offset`/`shadow_spread`,
  `outline`, `text_shadow`
- **Misc**: `overflow`, `overflow_clip_margin`, `zindex`, `global_zindex`,
  `font` (custom font asset), `pickable`, `id`, `target`, `watch`

(So e.g. `hover:background="#222"` parses but the hover color won't change yet
— use a script-driven binding for state styling until transitions land.)

---

## Minimal examples

**Static panel**
```html
<template>
  <node position="absolute" top="20px" left="20px" padding="14px"
        background="#11151C" border="1px" border_color="#222B38" border_radius="10px">
    <text font_size="14" font_color="#FFFFFF">Hello</text>
  </node>
</template>
```

**Live HUD bound to a component**
```html
<template>
  <node position="absolute" bottom="20px" left="20px" flex_direction="column">
    <text font_color="#FFFFFF">{{ Player.Stats.name }}</text>
    <text font_color="#F1C40F">Lv {{ Player.Stats.level }}</text>
    <node show="{{ Player.Health.current < 20 }}"><text font_color="#E74C3C">DANGER</text></node>
  </node>
</template>
```

**Custom cursor** (spawn once from a script)
```html
<template>
  <image name="cursor_follow" src="images/cursor.png"
         position="absolute" width="32px" height="32px" />
</template>
```
