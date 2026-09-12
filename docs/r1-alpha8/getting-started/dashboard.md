# The Dashboard

Start Renzora without a project and you land on the dashboard. It is where you pick a project, install plugins, and take engine updates.

<!-- screenshot: dashboard_projects.png - the dashboard on its Projects page, recent project cards in a grid, rail on the left -->

The window has a rail down the left with one row per page, your account and language picker pinned at the bottom, and a status strip along the bottom with links to the website, YouTube, Discord and GitHub. Drag the window by its title bar.

## Projects

The page you land on.

| Button | What it does |
|---|---|
| New Project | Pick a folder. It becomes the project root and takes the folder's name. |
| New from Template | Start from a finished project instead of an empty one. See below. |
| Open Project | Pick a `project.toml`. |

Below those, **Recent Projects** lists everything you have opened, newest first. Click a card to open it. The **✕** on a card removes it from the list and does not touch the folder on disk. A project whose folder has moved is greyed out and marked `(missing)`. The search box filters by name or path.

Each card shows the last picture the editor took of that project's main scene, which it takes from the viewport every time you save. A project you have never saved keeps a folder icon instead.

## Templates

A template is a whole project, already finished. You pick a template, you pick a folder, and what lands there is a project. It opens straight away, and from then on it is no different from one you made yourself.

Templates come from the Marketplace, so this page is a browse with search, thumbnails and descriptions. Free templates need no account. A paid one needs you signed in.

A template brings its own `project.toml`, so it can start you with a resolution, a rendering mode or an audio bus layout already set. Only the project name changes, to the folder you chose.

<!-- screenshot: dashboard_templates.png - the Templates page, template cards with thumbnails -->

## Plugins

Install plugins here, before you open a project.

A plugin is compiled and loaded when the editor starts, so installing one always ends with a restart. On the dashboard that restart costs nothing, which is why the page is here.

<!-- screenshot: dashboard_plugins.png - the Plugins page, listings with Install buttons -->

The list you land on is the official plugins. Search reaches the whole catalogue. Each listing shows its version, its price and one action.

| Button | Meaning |
|---|---|
| Install | Not installed. Free plugins install without an account. |
| Update | Installed, and a different version is published. |
| Installed | Installed, at the published version. |
| Needs newer engine | A newer version exists but needs an engine release this build is behind. Update the editor first. |
| Sign in | The listing is paid and you are not signed in. |

When an install finishes a strip appears offering **Restart now**.

If a different plugin already occupies the same name, the new one installs under `name_2` and the message says so.

## Updates

The same updater the editor opens from **Help > Check for Updates**, as a page. It shows the version you are on, the release channel, every version that channel offers, where the install goes, and the button that installs it.

Taking an update here is cheaper than taking it in the editor, because there is no project open and no scene to lose.

See [Installation](/docs/r1-alpha8/getting-started/installation#keeping-it-up-to-date) for what the channels mean.

## Changelog

Every release of the engine, newest first, with its notes and a link to the release. The build you are running is tagged **This build**. Prereleases are marked as such.

The list is fetched once per launch. If GitHub cannot be reached the page says so.

## Your account

The bottom of the rail is your renzora.com account. Signed out it is a **Sign in** button, and signing in here carries into the editor session that follows. Signed in it shows your profile picture and username with a sign-out control.

You need an account only for paid marketplace listings and for publishing. Free plugins and free templates install without one.

## Language

The last row of the rail picks the interface language. The choice is saved, takes effect on the dashboard immediately, and is already applied when the editor opens.
