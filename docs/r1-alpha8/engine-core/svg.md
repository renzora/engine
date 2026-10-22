# SVG Images

`.svg` and `.svgz` files load as ordinary images. Anywhere the engine takes an image (a particle texture, a material texture slot, a UI image, a sprite, a tileset, an asset-browser thumbnail) takes an SVG too, and an SVG you edit while the editor runs updates in place.

## How it works

The `renzora_svg` plugin registers a Bevy asset loader for `.svg` and `.svgz`. When something loads one, the loader rasterizes it with [resvg](https://github.com/linebender/resvg) (a pure-Rust renderer, so it works on every platform the engine ships, the web included) and hands back a normal `Image`. Nothing that uses the image knows it came from a vector file:

```rust
let icon: Handle<Image> = asset_server.load("ui/icons/heart.svg");
```

SVGs are **not** converted when imported. They are copied into the project as they are and rasterized each time they load, so the vector file stays the one source of truth: there is no baked copy to keep in sync, and changing the output size is a settings edit rather than a re-import.

## Settings

With no settings, an SVG rasterizes at its own size (its `width`/`height`, or its `viewBox` when those are missing), with the longest side capped at 4096 pixels.

To change that, put a TOML file next to the SVG named after it with `.toml` added: `heart.svg` takes its settings from `heart.svg.toml`. Every key is optional.

```toml
# ui/icons/heart.svg.toml
width = 128              # height follows the aspect ratio
mipmaps = true
fonts = ["/fonts/Brand-Bold.ttf"]
```

| Key | Default | What it does |
|---|---|---|
| `width` | none | Output width in pixels. On its own, the height keeps the SVG's aspect ratio. |
| `height` | none | Output height in pixels. With `width` as well, the image is stretched to exactly that size. |
| `scale` | `1.0` | Multiplies the SVG's own size. Used only when `width` and `height` are both absent; `2.0` is a good choice for UI icons on high-DPI screens. |
| `max_size` | `4096` | The longest side is scaled down to fit this, keeping the aspect ratio. It can be raised to at most 8192, the largest texture every GPU accepts. |
| `srgb` | `true` | Store the colours as sRGB. Turn it off for an SVG a shader reads as data (a mask, a flow map). The loader's `ImageLoaderSettings::is_srgb` is the default when this key is absent. |
| `mipmaps` | `false` | Build a full mip chain. Each level is drawn from the vector data at that level's size, so small mips stay sharp. Turn it on for a texture seen at a distance, such as a material or a particle that shrinks; UI and sprites draw near full size and do not need it. |
| `premultiplied_alpha` | `false` | Output premultiplied alpha. Leave it off: Bevy images, like PNGs, use straight alpha. Only a material with `AlphaMode::Premultiplied` wants this on. |
| `fonts` | `[]` | Extra `.ttf`/`.otf` files for the SVG's text. See [Fonts](#fonts). |

An unknown key is an error, so a typo such as `widht` is reported rather than ignored. A settings file that does not parse is a warning in the Console, and the SVG loads with the defaults.

The background is always transparent.

> **Why a sidecar file and not a Bevy `.meta`?** The engine's asset reader does not serve `.meta` files, and the sprite, tileset and UI code load images with Bevy's `ImageLoaderSettings` to choose a sampler. Keeping that settings type is what lets those features accept an SVG without changes, so the options only a vector file has live beside it instead.

## Images inside an SVG

An `<image>` element (or a filter's `<feImage>`) can embed a picture in three ways, and all of them work:

- **A `data:` URI**: `href="data:image/png;base64,..."`. The picture travels inside the SVG.
- **A path relative to the SVG**: `href="img/badge.png"` next to `ui/logo.svg` reads `ui/img/badge.png`.
- **A path from the project root**: `href="/textures/badge.png"`.

PNG, JPEG, GIF and WebP pictures are supported, and so is another SVG (which, as the SVG standard requires, does not load further files of its own).

Files are read through the engine's asset reader, the same way every other asset is, so they work from an exported game's `.rpak` archive and on the web.

Each embedded file is also a **hot-reload dependency**: edit `badge.png` while the editor runs and every SVG that embeds it is drawn again. The same goes for the settings file.

A reference is **refused**, with a warning in the Console, when it:

- climbs out of the project with `../`,
- is a URL (`http:`, `https:`, `file:` and so on; nothing is ever fetched from the network),
- is a drive or network path (`C:/...`, `//server/...`).

A refused or missing picture is left out and the rest of the SVG still draws.

An SVG opened from outside any project (by absolute path) can only reach files in its own folder and below.

## Fonts

Text in an SVG never uses the fonts installed on the computer. If it did, the same SVG would look different on every machine, and on the web there are no installed fonts at all. Text is drawn with:

1. **Noto Sans**, built into the engine. It is also what `serif`, `sans-serif`, `monospace`, `cursive` and `fantasy` mean. It is licensed under the SIL Open Font License 1.1; the licence text ships beside it in `crates/renzora_svg/embedded/OFL.txt`.
2. **Your project's fonts**: every file listed under `fonts` in the settings file, and, for each `font-family` the SVG names, a matching file found in the project's `fonts/` folder or beside the SVG. For `font-family="Open Sans"` the loader looks for `Open Sans.ttf`, `OpenSans.ttf`, `Open-Sans.ttf`, each also with `-Regular`, as `.ttf` then `.otf`.

**Text whose font cannot be found is drawn in Noto Sans** rather than disappearing. Characters Noto Sans does not have (for example Chinese, Japanese or emoji) are skipped unless your project supplies a font that has them. A font file you add after the SVG has loaded is picked up the next time the SVG is saved.

## Exporting

An exported game ships each SVG a scene (or anything else) references, together with its settings file, every picture it embeds by path, and the project fonts it found by family name. `.svgz` files are unpacked during the export so their references are found too.

## When loading fails

A file that is not valid SVG (broken XML, no size, not UTF-8 text) fails to load. The error, with the file's path, appears in the **Console** under `SVG`, and whatever uses the image shows nothing rather than crashing. Fix the file and save it, and it loads.

## Limitations

- **No animation.** SMIL (`<animate>`) and CSS animations are ignored; the SVG draws as its first frame.
- **No scripting.** `<script>` and event attributes are ignored.
- **Filters** follow resvg's support, which covers the SVG 1.1 filter set (blur, offsets, lighting, displacement, turbulence and the rest) but not CSS filter functions beyond what resvg implements.
- **No external `<use>` references** (`href="other.svg#icon"`); a `<use>` must point inside the same document.
- **No external CSS** (`<?xml-stylesheet?>`, `@import`) and no `@font-face`; fonts come only from the places in [Fonts](#fonts).
- **Fixed resolution once loaded.** The image is a raster at the size its settings give; scaling a UI node up past that size blurs it like any image. Raise `width` or `scale` for an image drawn larger.
- **Hot reload of new files.** A settings file, picture or font that did not exist when the SVG loaded is not watched; save the SVG to pick it up.
- **Import copies only the SVG.** Importing an SVG that references pictures by path copies the SVG alone. Copy the pictures into the project too, or embed them as `data:` URIs.

## For plugin authors

- `renzora::IMAGE_EXTENSIONS` lists every extension that loads as an `Image`, SVG included, and `renzora::is_image_extension` / `renzora::is_image_path` test against it. Use these for a texture slot, a drop target or a file picker instead of writing a list, so the next format the engine learns arrives in your feature for free. `renzora::VECTOR_IMAGE_EXTENSIONS` is the SVG subset.
- `renzora_svg::SvgRasterSettings` is the settings file's schema, and `renzora_svg::raster::rasterize` rasterizes SVG text without the asset server, for tools that want to.
