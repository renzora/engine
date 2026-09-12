# Installation

Download Renzora, run it, and you are in the editor in a few minutes.

![The Renzora editor with a 3D city scene open: a scene list on the left, the viewport with move and rotate gizmos in the middle, the Inspector on the right, and an asset browser along the bottom.](/assets/previews/interface.png)

## System requirements

| | Minimum |
|---|---|
| Windows | Windows 10, 64-bit |
| macOS | macOS 12 Monterey |
| Linux | Ubuntu 22.04 or Fedora 38 |
| Graphics | A GPU with Vulkan, Metal or DX12 |
| Memory | 4 GB, 8 GB recommended |

## Download the editor

Grab a build for your platform from [renzora.com/download](/download). Nothing else to install.

**Windows.** Extract the `.zip` anywhere and run `renzora.exe`.

**macOS.** Extract the `.zip` and move the app to your Applications folder. The app is signed but not notarized, so the first launch is blocked. Open **System Settings > Privacy & Security**, find the message naming Renzora Engine, and click **Open Anyway**. You only do this once.

**Linux.** Extract the `.zip` and run the `.AppImage` inside it.

### Nightly builds

Alongside numbered releases there are nightly builds, tagged like `r1-alpha8-nightly-16aug26`. They carry the newest fixes and the newest bugs. Use one if you are working closely with engine changes. Do not ship a game on one.

## Keeping it up to date

The editor updates itself. **Help > Check for Updates** downloads the new version and installs it in place. When a startup check has already found one, the menu item names the version instead, and an update chip appears in the top bar.

The dialog walks you through Download, then Install and Restart. The download is checksummed, and if anything goes wrong while files are being replaced your existing install is put back.

| Option | What it does |
|---|---|
| Install to | The folder that gets replaced. Defaults to where the editor is running from. |
| Skip This Version | Stops the top bar mentioning the version on offer. The next release asks again. |
| Channel | **Auto** follows what you are running, **Stable** offers releases only, **Nightly** offers dated builds of `main`. |

Nightly requires Dev Mode, which sits under the channel picker and in **Settings > Editor**. With Dev Mode off, every channel resolves to Stable.

## What's next

- [The Dashboard](/docs/r1-alpha8/getting-started/dashboard) is the first screen you see.
- [Core Concepts](/docs/r1-alpha8/getting-started/concepts) explains scenes, entities and components.
- [Your First Project](/docs/r1-alpha8/getting-started/first-project) builds something.
