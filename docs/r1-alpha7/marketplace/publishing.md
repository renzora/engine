# Publishing Assets

List your models, scripts, audio, plugins, and more on the Renzora Marketplace through the publish wizard — either **on renzora.com** or **inside the editor** (see [Publishing from the editor](#publishing-from-the-editor)). Both flows are the same six steps and hit the same API, so this page describes them together.

## Become a creator first

Before you can sell, finish the creator setup at [Marketplace → Sell](/marketplace/sell) (the `/marketplace/sell` route). It is a three-step onboarding:

1. **Accept the Marketplace Creator Agreement** — confirms you own the rights to what you upload, agree to the 80/20 revenue split, and follow the content rules.
2. **Connect a payout account** — payouts run on **Stripe Connect** (Stripe Express onboarding). This is required to receive money from *paid* assets. You can **skip it** and still publish — you'll just be limited to free assets until you connect Stripe later in [Settings](/settings).
3. **Start selling** — you're sent to the upload wizard.

> The marketplace uses **Stripe Connect** for payouts, not PayPal or manual bank transfer. The minimum withdrawal is **500 credits ($50)** — see [Credits System](./credits) for the payout details.

Uploading your first piece of content also automatically upgrades your account role to **creator**; you don't have to apply separately.

## The publish wizard

Everything is published through one wizard at [Marketplace → Upload](/marketplace/upload). You must be signed in (the page shows a sign-in gate otherwise). It's a six-step flow with a progress bar:

| Step | What you do |
|---|---|
| **1 — Content Type** | Choose **Marketplace Asset** or **Game**. (This page covers assets. Games use the same wizard but post to the game store and ask for platforms and system requirements instead of tags.) |
| **2 — Category** | Pick a category. The list is loaded live from `/api/marketplace/categories`, so it can change over time. |
| **3 — Basic Information** | Name, description, version, price — plus tags, download filename, and attribution for assets. |
| **4 — Additional Details** | Category-specific fields (see below). Some categories need nothing here. |
| **5 — Files & Media** | Your asset file, a cover image, screenshots, and optional video/audio previews. |
| **6 — Review & Publish** | Check the summary, then **Publish**. |

> There is **no draft step and no review queue**. Clicking **Publish** uploads your files and makes the asset **live on the marketplace immediately**. (You can later unpublish it from the edit page — see below.)

## Publishing from the editor

You don't have to leave the engine to publish. The editor ships a **Publish** panel that is the same six-step wizard, laid out identically to the website.

**Opening it:**

- In the **Marketplace** panel's left column, click **Upload Asset**.
- Or open the **command palette** (`Ctrl`/`Cmd` + `P`) and run **Open Publish**.

The panel docks like any other, so you can keep it beside the viewport while you prepare files. You must be **signed in** (sign in from the Marketplace panel first); publishing to a *paid* price also needs a connected payout account, exactly as on the web.

**How it differs from the website (only in mechanics, not in fields):**

- File inputs open a **native file picker** rather than a browser file dialog — one for the main file, the cover image, screenshots (multi-select), and audio previews.
- The same **content type → category → basic info → details → files & media → review** steps apply, with the same required fields, the same "credited assets are free" rule, and the same live tag autocomplete / submit-a-new-tag behaviour.
- On success the panel shows a **View →** link that opens the published asset (or game) page in your browser.

Everything below — field rules, category-specific details, size limits, categories, and pricing — applies to both the website and the editor panel.

### Step 3 — Basic information

| Field | Rules |
|---|---|
| **Name** | Required. 1–128 characters. |
| **Description** | Required. 1–5000 characters. Plain text — Markdown is **not** rendered. |
| **Version** | A version string, defaults to `1.0.0` (1–32 characters). |
| **Price** | In **credits**. `0` = free. See [Pricing](#pricing) below. |
| **Tags** | Assets only. Up to **5** tags, lowercase, each ≤ 32 characters. |
| **Download Filename** | Assets only. The filename buyers see when downloading; auto-filled from your uploaded file. |
| **Credit / Attribution** | Assets only. If the asset is built from another creator's work, name them and link the source. |

> **Tag review:** typing a tag that doesn't exist offers to *submit it as a new tag*. New tags are created in a pending state and only appear in everyone's autocomplete **after a moderator approves them** — your asset still keeps the tag in the meantime.

> **Attribution forces free:** if you fill in an *Original Creator Name*, the price is locked to **0 credits**. Credited (re-distributed) assets must be free.

### Step 4 — Category-specific details

The wizard shows extra fields based on your content type and category, for example:

- **Sound Effects / Music** — BPM, genre, loop-friendly flag.
- **Scripts / Plugins / Blueprints** — scripting language (Lua, Rhai, WGSL, Visual Blueprint) and dependencies.
- **3D Models / Animations** — polygon count and texture resolution.
- **2D Art / Textures / Particles** — resolution and a "seamlessly tileable" flag.
- **Materials & Shaders** — render pipeline (PBR, Unlit, Toon, Custom WGSL) and texture resolution.
- **All assets** — an "AI-assisted" checkbox, supported engine versions, and a license choice.

If your category doesn't need any of these, the step shows "No additional details needed" and you continue.

### Step 5 — Files & media

| Upload | Limit |
|---|---|
| **Asset file** (required) | **200 MB** per file (server-enforced). |
| **Cover image** | Recommended **1280×720** (16:9), PNG or JPG. Max 10 MB. |
| **Screenshots** | Up to **10** images, max 10 MB each. |
| **Video preview** (optional) | A URL — YouTube links auto-embed, or a direct `.mp4`. |
| **Audio previews** (optional) | MP3 / WAV / OGG / FLAC, max 50 MB each. |

> The upload endpoint enforces a hard **200 MB per file** cap, even though some categories advertise a larger recommended size in the table below. For anything bigger, split it across multiple files or trim the package.

## Categories

Categories are defined server-side. The current set (with the recommended max size and the file types each accepts):

| Category | Slug | Rec. max | Accepted formats |
|---|---|---|---|
| 3D Models | `3d-models` | 100 MB | zip, rar, 7z, fbx, obj, gltf, glb, blend |
| Animations | `animations` | 100 MB | zip, rar, 7z, fbx, bvh |
| Materials & Shaders | `materials` | 50 MB | zip, rar, material, wgsl |
| Textures & HDRIs | `textures` | 200 MB | zip, rar, png, jpg, hdr, exr |
| 2D Art & Sprites | `2d-art` | 50 MB | zip, rar, png, svg, psd, aseprite |
| Particle Effects | `particles` | 50 MB | zip, rar, 7z |
| Sound Effects | `sfx` | 100 MB | zip, rar, wav, ogg, mp3, flac |
| Music | `music` | 200 MB | zip, rar, wav, ogg, mp3, flac |
| Plugins | `plugins` | 50 MB | zip, rar, 7z |
| Scripts | `scripts` | 50 MB | zip, rar, lua, rhai |
| Blueprints | `blueprints` | 50 MB | zip, rar, blueprint |
| Complete Projects | `projects` | 500 MB | zip, rar, 7z |
| Themes | `themes` | 20 MB | zip, rar, json |
| Fonts | `fonts` | 20 MB | zip, rar, ttf, otf, woff, woff2 |

> Accepting an upload format is not the same as the engine loading it at runtime. Renzora loads `.glb`/`.gltf`, `.png`/`.jpg`/`.hdr`, `.lua`/`.rhai`, `.ron`, `.material`, `.particle`, and `.ogg`/`.mp3`/`.wav`/`.flac` directly; other model formats convert to GLB at import, and `.exr` is **not** a supported runtime texture. See [Browsing & Installing](./browsing) for the full runtime-format note.

## Pricing

Prices are set in **credits**. **1 credit = $0.10 USD**.

- **Free** — set the price to `0`. Anyone can download it. Great for building a following.
- **Paid** — set any positive credit amount. You **keep 80%** of each sale; Renzora takes a **20% platform fee**.
- **Credited assets are always free** — filling in an attribution credit locks the price to 0.

> Example: list an asset for **500 credits ($50)**. Each sale earns you **400 credits ($40)**; the 100-credit fee covers hosting and payment processing.

You can change the price at any time from the edit page. For how buyers pay, payouts, promo codes, and refunds, see [Credits System](./credits).

## After you publish

Your asset goes live right away at `/marketplace/asset/<slug>` and starts appearing in browse and search. There is no pre-publication approval gate, but the Creator Agreement still applies:

- You must own or have rights to everything you upload.
- **No malware, no IP infringement, no illegal or harmful content.**
- Listings must be accurate, functional, and reasonably documented.

Renzora can remove content that violates these terms, and buyers can open **refund disputes** for non-functional or misrepresented assets (approved refunds are deducted from your balance). Repeated violations can suspend your creator account.

## Editing and updating

Open your asset and choose **Edit** (the `/marketplace/asset/<slug>/edit` route — only the owner can open it). From there you can:

- Change the **name, description, version, and price**.
- Replace the **thumbnail**.
- Replace the **asset files** — upload several files, or a single `.zip` with a choice to **keep it as a zip** or **extract its contents**. Replacing files swaps out *all* current files. Max 200 MB each.
- Add or remove **gallery media** (screenshots and video URLs).
- Toggle **Published** on or off — unpublishing hides the asset from the marketplace without deleting it.

> **Category can't be changed** after creation — it's locked in the edit form.

> There is **no version history and no per-version changelog**. An asset carries a single version string and an *Updated* date; uploading new files overwrites the existing download. (This corrects older docs that promised preserved version history.)

## Tracking performance

Your [Dashboard](/dashboard) is where you manage published content. The header shows four totals — **Assets**, **Downloads**, **Earnings** (credits), and **Balance** (credits) — and the tabs below let you:

- Browse your **Assets** and **Games**, each row showing its **downloads**, **views**, and a **Published / Draft** status dot.
- Open the **Earnings** tab for your credit transaction history.

> The dashboard reports per-asset downloads and views plus your earnings and balance. It does **not** include time-series charts or geographic breakdowns.

## Best practices

- **Use a clean cover image** — 16:9 at 1280×720, showing the asset in a real scene rather than a gray void.
- **Add screenshots** — up to 10; they populate the gallery on your asset page.
- **Write an accurate description** — say exactly what's included, the formats, and any dependencies. It's plain text, so keep it scannable.
- **Test in a fresh project** before uploading so nothing is missing from the package.
- **Credit your sources** — if you redistribute someone else's work, attribute them (and remember it publishes free).

## Related

- [Browsing & Installing](./browsing) — how buyers find and install your asset.
- [Credits System](./credits) — pricing, payouts, promo codes, and refunds.
