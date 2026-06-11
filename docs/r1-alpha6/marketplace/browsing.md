# Browsing & Installing Assets

Need a chair, a sound effect, or a ready-made script for your game? The Renzora marketplace is full of community-made assets, and you can drop the ones you own straight into your project.

## Two ways to browse

You can shop the same catalog in two places:

- **On the website** — visit [renzora.com/marketplace](/marketplace) to browse, preview, buy, and read reviews.
- **Inside the editor** — open the **Hub Store** tab to browse and install without ever leaving your project.

![The in-editor Hub Store: a grid of asset cards — here mostly free wooden props plus a scripts pack — with a search box at the top.](/assets/previews/marketplace.png)

> To install assets you own, sign in to your renzora.com account in the editor. Signed out, you can still look around the store, but your personal library won't show up.

## Finding what you need

Both views give you the basics: a **search box**, a list of **categories** down the side, and a way to **sort** results (newest, most popular, top rated, or by price).

The website adds a few more options when you want to narrow things down — filter by **price** (free or paid), **minimum star rating**, **license**, or a **tag**, and switch between grid and list views. The in-editor store is a lighter, browse-only version; when you find something you want to buy, head to the website to complete the purchase.

Categories aren't fixed — the list is set by Renzora and can grow over time, so check back as new types of assets appear.

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

### From the editor (easiest)

1. Open the **My Library** panel in the Hub. It lists everything you own and shows which folder each asset installs into.
2. Click **Install** on the asset you want.

The editor downloads the asset and writes it into the correct folder for you. When it's done, you'll see a short "Installed" message. If a new asset doesn't show up right away, reopen (or rescan) your project so the editor notices the new files.

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
