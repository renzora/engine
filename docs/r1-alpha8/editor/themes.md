# Themes

A theme is the editor's whole look: its colours, its fonts, and optionally a background shader. Renzora ships **Dark**, **Light** and around a hundred more.

Pick one under **Settings > Theme**.

<!-- screenshot: settings_theme.png - the Theme page with the theme picker and the colour sections -->

## Picking a theme

**Active Theme** lists everything available. Choosing one applies it immediately, across the whole editor. There is nothing to restart and nothing to confirm.

A theme can carry its own UI font, so switching may change more than colour.

## Editing a theme

Below the picker, the theme's colours are laid out in sections.

| Section | Covers |
|---|---|
| Semantic Colors | Accent, success, warning, error. The meanings, not the places. |
| Surfaces | Panel backgrounds, dividers, the editor's overall ground. |
| Text | Primary, muted, placeholder and disabled text. |
| Widgets | Buttons, inputs, sliders and their hover and pressed states. |
| Panels | Per-panel chrome: tabs, headers, the dock. |
| Syntax Tokens | The code editor's highlighting. |
| Editor Chrome | The top bar, status bar and ribbon. |
| Widget Styles | Per-widget shape: corner radius, border width, padding. |

Edits apply live as you make them. The theme is marked as having unsaved changes until you save it, and saving writes a new theme file rather than overwriting a built-in.

## Where themes live

Themes are files in your themes directory, and come in two shapes.

A simple theme is a single `.toml` file:

```text
themes/
└── My Theme.toml
```

A theme that ships its own font or background shader is a folder:

```text
themes/
└── My Theme/
    ├── theme.toml
    ├── background.wgsl
    └── doto.ttf
```

`theme.toml` points at the font and shader by name, relative to the folder. Editing either one while the editor is running reloads it, so you can tune a background shader with the editor open in front of you.

## Sharing a theme

A theme is a file or a folder, so sharing one is sending it. Drop what you were given into your themes directory and it appears in the picker.

Themes are also published on the [Marketplace](/docs/r1-alpha8/marketplace/browsing), which installs them for you.

## Themes are not part of your game

A theme styles the editor. It does not travel with a project and it has no effect on what a player sees. Your game's own interface is built with [Game UI](/docs/r1-alpha8/scripting/game-ui), which has its own styling.
