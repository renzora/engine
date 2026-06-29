# Browsing & Installing Assets

Need a chair, a sound effect, or a ready-made script for your game? The Renzora marketplace is full of community-made assets, and you can drop the ones you own straight into your project.

## Two ways to browse

You can shop the same catalog in two places:

- **On the website** — visit [renzora.com/marketplace](/marketplace) to browse, preview, buy, and read reviews.
- **Inside the editor** — open the **Marketplace** tab to browse, preview, and install without ever leaving your project.

![The in-editor Marketplace: a left column with your account and categories, a search/sort toolbar, and a grid of asset cards each with a download button.](/assets/previews/marketplace.png)

> To install assets you own, sign in to your renzora.com account in the editor. The **left column** shows who you're signed in as and your **credit balance**; signed out, it shows a **Sign In** button that opens the sign-in window. You can still look around the store while signed out — free assets install without an account, but paid downloads and your personal library need you signed in.

## The left column

Down the left side of the Marketplace tab you'll find:

- Your **account** — "Signed in as …" and your current **credit balance**, or a **Sign In** button when you're signed out.
- **Upload Asset** — a shortcut for publishing your own work (coming soon).
- The **category** list — click one to filter the grid; **All** clears the filter.

## Finding what you need

Both views give you the basics: a **search box**, the **categories** in the left column, and a **sort** dropdown (newest, most popular, or by price low/high).

The website adds a few more options when you want to narrow things down — filter by **price** (free or paid), **minimum star rating**, **license**, or a **tag**, and switch between grid and list views.

Categories aren't fixed — the list is set by Renzora and can grow over time, so check back as new types of assets appear.

## Previewing a theme live

For **theme** assets, each card has a **Preview** button. Click it and Renzora downloads the theme and applies it to the editor right away — no install, no commitment — so you can see your panels, colors, and accents in the real UI. A banner across the top of the tab shows what you're previewing with two choices: **Install Theme** to keep it (this opens the normal install prompt) or **Stop** to snap straight back to the theme you had.

## Buying an asset

Purchases use **credits** (1 credit = $0.10 USD). You buy credit packs from the [Credits page](/wallet) — see [Credits System](./credits) for the details.

On an asset's page, the button changes to match your situation:

- **Free asset** → **Download for Free**
- **Paid asset** → enter a promo code (if you have one) and **Buy for X credits**
- **Already owned** → **Download** and **Show in Library**

Buying adds the asset to your **Library** so you can install it any time. If you don't have enough credits, top up first.

## Where assets land in your project

When you install an asset, Renzora puts its files in the right folder for you. A few common examples:

| Asset type | Goes into |
|---|---|
| 3D models | `models/` |
| Textures | `textures/` |
| Audio (music, SFX) | `audio/` |
| Scripts | `scripts/` |
| Scenes | `scenes/` |
| Blueprints | `blueprints/` |
| UI themes | `themes/` |

Anything that doesn't match a known type lands in a general `assets/` folder. You don't have to memorize this — the editor sorts it out automatically.

## Installing into your project

### Straight from a card (easiest)

Every asset card has a **Get** (free) or **Buy** (paid) button. Click it and Renzora opens an **install prompt** that works like installing a plugin from file — it shows you what's being installed and asks **where to put it**:

1. Pick a destination from the **folder tree**, which mirrors your project's own folders. It defaults to the conventional folder for that asset type (see the table above), but you can drop the files anywhere — for example into a specific subfolder of `models/`.
2. Read the short note (Renzora only downloads and writes files into the folder you choose — only install assets from sources you trust).
3. Click **Download & Install**.

The asset downloads in the background and a notice confirms where it landed. A paid asset you're not signed in for sends you to the sign-in window first.

### From your Library

1. Open the **My Library** panel. It lists everything you own and shows which folder each asset installs into.
2. Click **Install** on the asset you want.

If a new asset doesn't show up right away, reopen (or rescan) your project so the editor notices the new files.

### Manually

Prefer to do it by hand? You can:

1. Click **Download** on the [website Library](/library) or the asset's page. Multi-file assets offer a single **Download All (.zip)**.
2. Unzip and drop the files into the matching project folder (see the table above).
3. Reload your project so it picks up the new files.

## Previewing before you buy

On the website, click any asset card to open its page, where you can see images, play audio, and watch video. For 3D models, materials, textures, and particle effects, a **Live Preview (BETA)** renders the asset right in your browser so you can spin it around before deciding. Each page also shows the star rating, downloads, tags, and the publish/updated dates.

## Updating and removing

- **Update** — click **Install** again to re-download the latest version into your project.
- **Remove** — delete the installed files from your project folder.

## Rating and reviews

If you're signed in, own the asset (or it's free), and aren't the creator, you can leave a review on its page:

1. Pick **1–5 stars** with the star picker.
2. Optionally add a title and a few words, then **Submit Review**.

There's also a quick star widget for a rating-only vote, plus a **Comments** thread for questions and discussion.

## Related

- [Publishing Assets](./publishing) — list and sell your own work.
- [Credits System](./credits) — how credits and pricing work.
