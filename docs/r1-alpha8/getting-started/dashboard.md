# The Dashboard

Start Renzora without a project and you land on the **dashboard** — the window that runs before the editor does. It is where you pick a project, but it is also the only place in the app where the engine itself is not yet running, which makes it the right place to do the things that only take effect on the next start.

The Light Chamber animation behind it is not decoration you have to look past: every surface on the dashboard is translucent over it.

## The layout

| Where | What |
|---|---|
| **Title bar** | The Renzora mark, the engine version you are running, the window's drag handle, and the minimize / maximize / close buttons. |
| **Rail** (left) | One row per page, with your account and the language picker pinned to the bottom. |
| **Page** | Whichever rail row is selected. |
| **Status strip** (bottom) | Frame rate, and links to the website, YouTube, Discord and GitHub. |

Dragging the window is the title bar's job only. A press anywhere else — the rail's empty space, a page's background — does nothing, so a click that misses a button no longer picks the window up.

The window opens at about half your screen, centred: the dashboard is a launcher, and a launcher that fills the display is claiming more than it needs. Resize or maximize it if you like — and when you open a project, it maximizes itself, because at that point it *is* the editor.

## Projects

The default page, and what the launcher used to be on its own.

- **New Project** — choose a folder; it becomes the project root and takes the folder's name.
- **Open Project** — pick a `project.toml`.
- **Recent Projects** — everything you have opened, newest first. Click a row to open it. The **✕** on a row removes it from this list and *does not* touch the folder on disk. A project whose folder has moved or gone is shown greyed out and marked `(missing)`.
- The search box filters the recents list by name or path.

In the browser build, "New" and "Open" both go through the directory picker, and a recent entry reopens through the folder handle the browser remembers — which asks you to re-grant permission.

## Plugins

Install a plugin **before** you open a project.

This is not the Marketplace panel in miniature — there are no categories, filters or item pages here, just plugins and a search box. It exists because of where a plugin lives: a plugin extracts into the engine's own `plugins/` directory, is compiled by `prebuild` at startup and loaded by `NativePluginLoader` at startup. Installing one from inside the editor always ends with "it will be there next time you start". On the dashboard, next time you start is a button away.

**The list you land on is the official plugins.** That is what a launcher can reasonably offer unprompted — a plugin is compiled and loaded into the editor process at startup, which is a lot of trust to extend to something you did not ask for. **Search reaches the whole catalogue**, because typing a name is asking for it.

Each listing shows its version, its price and one action:

| Button | Meaning |
|---|---|
| **Install** | Not installed. Free plugins install without an account. |
| **Update** | Installed, and the marketplace publishes a different version. |
| **Installed** | Installed, at the published version. |
| **Needs newer engine** | There is a newer version, but it requires an engine release this build is behind. Update the editor first. |
| **Sign in** | The listing is paid and there is no session to download it with. |

When an install finishes, a strip appears offering **Restart now** — that restart is what compiles and loads what you just installed.

A plugin installs under its crate's name. If a *different* listing already occupies that name, it installs under `name_2` (and so on) instead, and the message says so — the plugin builds and loads under that name.

## Updates

The same updater the editor opens from **Help → Check for Updates**, as a page: the version you are on, the release channel (Auto / Stable / Nightly), every version the channel offers, where the install goes, and the button that downloads and installs it.

It is here because this is the cheapest moment to take an update. Installing swaps the files the editor is running out of — which is why the swap is handed to a sidecar and ends in a relaunch. On the splash there is no project open and no scene loaded, so the restart costs nothing; in the editor the same dialog is an interruption you have to weigh.

Two things worth knowing, both the same as in the editor:

- **Nightlies need developer mode.** With it off, every channel preference resolves to Stable. A nightly is last night's `main`.
- **Running from a source checkout?** Installing overwrites the `dist/` tree `cargo renzora` stages into, so the button says so and asks twice.

## Changelog

Every release of the engine, newest first, straight from the project's GitHub releases — including the notes, the date, and a link to the release itself. The release matching the build you are running is tagged **This build**; prereleases are marked as such.

The list is fetched once per launch. If GitHub cannot be reached, the page says so rather than showing an empty list, and the **All releases** link still works.

## Your account

The bottom of the rail is your renzora.com account.

- Signed out, it is a **Sign in** button. It opens the same sign-in window the editor's title bar does, so signing in here signs you in for the session that follows — you do not do it twice.
- Signed in, it shows your username, with a sign-out control beside it.

You only need an account for paid marketplace listings — and for publishing, which happens in the editor. Free plugins install without one.

If you are running a build with no marketplace in it, the account block is not shown at all.

## Language

The last row of the rail picks the interface language, from the built-in packs plus any `languages/*.toml` you have added. The choice is saved and is already in effect when the editor opens.
