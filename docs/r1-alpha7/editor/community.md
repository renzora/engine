# Community Panels

The **Hub** workspace connects the editor to renzora.com: the community feed, forum, messages, friends, docs, the marketplace, your **Wallet** (credits & donations), and creator onboarding — all as dockable panels (category *Community* in the panel list). Sign in from the account menu to activate them. Your renzora.com **account settings** (profile, email, password, social links, communication, connected apps) live in *Settings → Account*.

Two things that used to be panels no longer are, because they duplicated surfaces you already have:

- **Profiles** open as a shared **overlay** (a modal over whatever you're doing), so a username click anywhere pops the same view — see *Profiles* below.
- **Notifications** live only in the **top-bar bell** and its dropdown; there's no separate panel.
- **Teams** were folded into the **Friends panel** as a *Teams* tab — see *Teams* below.

## The feed

The feed is **one unified activity stream**: community posts, the forum's latest threads, and fresh marketplace assets, merged into one timeline.

**Live vs. Paused.** The header carries a **Live / Paused** toggle (top-right). While **Live** (the default), new posts announced over the site's WebSocket are pulled in automatically and appear at the top of the feed — no manual refresh. Click it to **Pause**; new posts then wait behind a "New posts — see what's fresh" pill you can click to catch up (or press the toggle to go Live again and pull them in at once).

**The filter bar** under the composer controls the stream:

- **Source chips** — Posts / Forum / Marketplace toggle each source on and off (accent = shown).
- **Sort** — *Recent* (newest first) or *Most popular* (likes + comments + reactions for posts; replies + views for threads).
- **Time frame** — All time / Today / This week / This month.
- **Audience** — Everyone / Following / Friends (filters by who authored it; your follow/friend lists load on first use).

Sorting and filtering are client-side over the fetched pages — the site API has no query parameters for them (a server wishlist item). Marketplace assets carry no timestamps, so they appear as a compact **"New in the Marketplace" strip** at the top rather than pretending to have a place in the timeline; clicking a card opens the Marketplace panel. Forum threads are compact cards ("alice started a thread in General") that open the thread in the Forum panel.

Each post is a card: author header (avatar, clickable username, role icon, time), then attached images, the post body, and one action row.

- **Reactions & likes are one row.** The ❤ chip toggles a like; the chips after it are reactions (click to join/leave one), and the ☺ chip opens the reaction picker. Chips light up with your theme accent when you're in them, and hovering a chip tells you how many people reacted and whether you're among them.
- **Comments** expand inline: click the 💬 chip to open the thread — each comment is a padded bubble with a clickable author name — and reply from the composer at the bottom (Enter sends).
- **Images** attach from the composer's image chip and lead the card, **above the text** — a single image renders near full width, multiples tile. **Click any image** to view it full-size in a dimmed lightbox overlay — click anywhere or press Esc to close.
- **Long posts collapse.** Bodies past ~500 characters are clamped with a **See more** link; click it to expand the full text in place (**See less** collapses it again).
- **Deleting**: your own posts show a trash chip in the header (site moderators see it on every post). Click it twice within a few seconds to confirm — the first click arms it.

## Profiles

Click any username — in the feed, comments, the forum, friends, chat, or a notification — to open that user's profile as a **centered overlay** over whatever you're doing. It's the *same overlay* everywhere; there is no profile panel to dock or lose. Click the dimmed backdrop, press **Esc**, or hit the ✕ to close; clicking another username swaps it in place.

The profile's **Activity** tab renders the user's posts with the *same cards as the feed* — likes, reactions, inline comments, image lightbox, and deletion all work identically there, and interacting in one place updates the other live. Opening a direct message from a profile closes the overlay so the chat panel isn't trapped behind it. Any **social links** the user has added show as an icon row.

**Editing your own profile.** Your own profile overlay has an **Edit profile** button. It opens an inline editor for your **avatar** (upload a photo) and **cover photo** (a wide banner image — or clear it to fall back to your banner *color*), plus **bio**, **location**, **website**, and **profile / banner colors** (`#rrggbb` hex fields). Avatar uploads cap at 2 MB, cover photos at 4 MB. Save writes to your renzora.com account and the profile refreshes in place.

- **Private notes**: every profile (other than your own) has a note field — "met at the game jam", "reported for spam", anything. Notes are stored locally in `~/.renzora/profile_notes.json` and are never sent to the server; think of them as moderation memory that follows the user, visible only to you.

## Teams

Teams live in the **Friends panel**, under a *Teams* tab (there's no standalone Teams panel). It shows your teams, any pending invites (Accept / Decline), and a **New team** field — team creation is **rate-limited** (a short cooldown between creates) so the button can't be spammed. Open a team to see its members, invite by username or email, and jump into its shared chat. Team notifications (invites, joins) deep-link straight to this tab.

## Notifications

Notifications are the **top-bar bell** and its dropdown — there is no notifications panel. The dropdown is **centered** under the bar, lists your most recent notifications newest-first (unread ones marked with a dot), and has a **Mark all read** action in its header. Clicking a notification takes you to its content: a mention or comment opens the feed with that post's comments expanded, a forum reply opens the thread, a follow opens the profile overlay, a team invite opens the Friends → Teams tab.

## The forum

The forum has three views — the **category list**, a category's **thread list**, and a single **thread** — with **clickable breadcrumbs** in the header (*Forum › Category › Thread*); click any earlier crumb to jump back. Thread rows show the author's avatar alongside the title and reply/view counts.

**Starting a thread** works from anywhere: the header's **New thread** button (shown when you're signed in) opens a centered composer with a **category dropdown** (pre-selected to the category you're in, if any), a title field, and a markdown body with a formatting toolbar. So you can start a thread straight from the main forum page and pick the category there, rather than having to drill into a category first. Backdrop-click or **Esc** cancels.

## The marketplace

The marketplace is a **storefront home** — a featured slider (arrows, dots, auto-advance) pinned at the top with a large **search bar directly beneath it**, over per-category shelves — plus a browse grid when you search or pick a category. Each card shows its artwork, price (Free / credits), creator, category, and downloads.

Clicking a card opens an **item page overlay** — a store product page over the editor:

- A **media gallery** (main viewer + thumbnail strip) of the asset's screenshots.
- A **native audio player** for music/SFX assets — play/pause, a scrub bar, and a waveform, streamed straight from the marketplace (no browser needed). The waveform **animates** as it plays: the portion left of the playhead lights up in your accent while the rest stays dim, so you can see progress at a glance.
- A **3D model viewer** for model/animation assets — the actual `.glb` is downloaded and rendered by the engine on a slowly-rotating turntable, right in the overlay. It's lit as a little studio (a three-point light rig) and sits on an infinite ground **grid** that drops to the model's feet and scales its spacing to the model, so the piece reads against a surface instead of floating in a void.
- A **live material / shader viewer** for materials & shaders — the engine downloads and compiles the shader, renders it on a **shape you pick** (sphere / cube / plane / torus), and turns the shader's `@param` annotations into **live controls** (sliders for numbers, RGB sliders for colours) that recompile the material in place as you drag — the same preview the website gives you, native in the editor.
- **Video** previews show a poster that opens in your browser (YouTube or a direct link).
- Creator, category and download count, the full description, a **star rating** you can set, an **Install / Get** button, and a **comments** thread (sign-in required to rate or comment).

Backdrop-click or **Esc** closes it. Installed **themes** show their real name in the theme picker (they used to appear as an internal id).

*(Coming next: native texture/HDRI and particle preview viewers rendered by the engine itself.)*

## Account settings

*Settings → Account* manages your renzora.com account without leaving the editor:

- **Profile & email** — change your username and email (email changes apply immediately, no re-verification).
- **Change password** — current + new (min 8 chars) + confirm, shown as masked fields.
- **Social links** — add links for Discord, Twitch, YouTube, Twitter/X, GitHub, Steam, Kick, Xbox, PlayStation, or Epic (pick the platform, enter your username/URL). OAuth-verified links show a check. Remove any with one click. These are what render on your profile.
- **Communication** — four **switches** for the emails you get: product updates, marketplace activity, comment replies, and security alerts.
- **Connected apps** — the third-party apps/games you've authorized to access your account, with the scopes they were granted and a **Revoke** button each.
- **Danger zone** — **delete your account** (password-confirmed, two-step). This permanently removes your account and signs you out.

Note: the *Social & Privacy* section (who can message you, online-status, profile visibility, forum signature, blocked users) is separate and unchanged — its boolean options are now **toggle switches** too.

## Wallet — credits & donations

The **Wallet** panel is your credit balance and everything money-related. Credits are the platform currency (**1 credit = $0.10**).

- **Buy credits** — pick a pack (50 = $5, 100 = $10, 250 = $25, 500 = $50) or a custom amount (min 50). Because card details can't be handled inside the editor, this **opens Stripe Checkout in your browser**; the credits land on your account once payment completes. You don't have to come back and refresh — when the payment clears, a live WebSocket event updates your balance in place and sets off a little **confetti** burst (the same happens for credits received as a gift).
- **Support Renzora** — donate credits to the platform: pick a preset (10 / 50 / 100 / 500) or a custom amount, add an optional message, and optionally donate **anonymously**. A live "credits donated" total and a **donor leaderboard** (top donors, medals for the top three) round it out, with Bronze/Silver/Gold/Platinum donor tiers at 100 / 500 / 1000 / 5000 credits.

## Become a Creator

The **Become a Creator** panel is the onboarding wizard for selling on the marketplace, in three steps:

1. **Creator Policy** — read the marketplace creator agreement (creators keep **80%** of each sale; payouts via Stripe Connect, minimum withdrawal 500 credits / $50) and accept it.
2. **Connect Payment Account** — connect your bank via **Stripe Connect** (opens in your browser) so you can receive payouts, or skip for now (required before selling *paid* assets).
3. **Start Selling** — jump to uploading your first asset.

The wizard reflects your live onboarding state; a **Refresh** button re-checks it after you return from the browser Stripe step. Once you're fully set up it shows an "all set" state instead of the steps.

## Searching the community

The command palette (`Ctrl+P`, or the magnifier in the top bar) has scope tabs for **Docs**, **Forum**, **Users**, **Feed**, **Courses**, and **Marketplace** — see *Keyboard Shortcuts → Command palette*.
