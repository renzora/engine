# Installing on Linux

Renzora ships for Linux as an AppImage: one file that runs on any distro with **glibc 2.28 or newer**, on x86_64 or ARM64, under Wayland or X11. Most desktops already have everything it needs. This page covers the ones that need a step or two more.

## Which systems are supported

| Distro | Minimum version |
|---|---|
| Ubuntu, Kubuntu, Xubuntu and other flavours | 20.04 |
| Linux Mint | 20 |
| Pop!\_OS | 20.04 |
| Debian | 10 |
| Fedora (including Silverblue, Kinoite, Bazzite) | 29 |
| RHEL, Rocky Linux, AlmaLinux | 8 |
| Arch, Manjaro, EndeavourOS, CachyOS | current |
| openSUSE Tumbleweed | current |
| Gentoo, Void (glibc) | current |
| SteamOS (Steam Deck) | 3 |
| NixOS | current, with one setting (see [NixOS](#nixos)) |

**Not supported:** distros built on musl instead of glibc (Alpine, Void musl, Chimera). See [musl distros](#alpine-and-other-musl-distros) for a way to run it anyway.

You also need a GPU with a Vulkan driver: Mesa for AMD and Intel, the proprietary driver for NVIDIA.

## Install

1. Download `linux-x64.zip` (or `linux-arm64.zip`) from [renzora.com/download](/download) and extract it.
2. Make the AppImage executable and run it:

   ```sh
   chmod +x Renzora*.AppImage
   ./Renzora*.AppImage
   ```

   From a file manager: right-click the AppImage, open **Properties > Permissions**, tick **Allow executing file as program**, then double-click it.

If something is missing, Renzora says what in a dialog and in the terminal, and the sections below say how to install it.

## Packages by distro

These are the libraries the editor loads. A normal desktop install already has nearly all of them, so run the line for your distro only if the startup message names something missing.

**Ubuntu, Mint, Pop!\_OS, Debian, elementary OS, Zorin OS**

```sh
sudo apt install libvulkan1 mesa-vulkan-drivers libxkbcommon0 libxkbcommon-x11-0 libasound2 libudev1 libfontconfig1
```

On NVIDIA, install the proprietary driver instead of relying on `mesa-vulkan-drivers` (on Ubuntu: **Software & Updates > Additional Drivers**).

**Fedora, RHEL, Rocky Linux, AlmaLinux**

```sh
sudo dnf install vulkan-loader mesa-vulkan-drivers libxkbcommon libxkbcommon-x11 alsa-lib systemd-libs fontconfig
```

**Arch, Manjaro, EndeavourOS, CachyOS**

```sh
sudo pacman -S --needed vulkan-icd-loader libxkbcommon libxkbcommon-x11 alsa-lib systemd-libs fontconfig
```

Plus the Vulkan driver for your GPU: `vulkan-radeon` (AMD), `vulkan-intel` (Intel) or `nvidia-utils` (NVIDIA).

**openSUSE Tumbleweed**

```sh
sudo zypper install libvulkan1 libvulkan_radeon libvulkan_intel libxkbcommon0 libxkbcommon-x11-0 libasound2 libudev1 libfontconfig1
```

**Gentoo**

```sh
sudo emerge --ask media-libs/vulkan-loader x11-libs/libxkbcommon media-libs/alsa-lib media-libs/fontconfig
```

Set `VIDEO_CARDS` for your GPU so Mesa builds its Vulkan driver.

**Void (glibc)**

```sh
sudo xbps-install vulkan-loader mesa-vulkan-radeon mesa-vulkan-intel libxkbcommon alsa-lib fontconfig
```

### Fedora Silverblue, Kinoite, Bazzite and other immutable distros

The libraries are part of the base image, so the AppImage usually runs as it is. If one is missing, add it with `rpm-ostree install <package>` and reboot, or run Renzora inside a [Distrobox](#alpine-and-other-musl-distros) container instead.

### Steam Deck (SteamOS)

Switch to **Desktop Mode**, download and extract the zip, then right-click the AppImage, choose **Properties > Permissions** and tick **Is executable**. Everything it needs is already in SteamOS.

## NixOS

NixOS does not run programs built for other distros on its own, because it has no `/lib64/ld-linux-x86-64.so.2` and keeps its libraries outside the usual paths. Pick one of these.

**Run it with `appimage-run`** (nothing to configure):

```sh
nix-shell -p appimage-run --run "appimage-run ./Renzora*.AppImage"
```

**Let NixOS run AppImages directly** (NixOS 24.05 and later), in `configuration.nix`:

```nix
programs.appimage = {
  enable = true;
  binfmt = true;
};
```

Run `sudo nixos-rebuild switch`, and from then on `./Renzora*.AppImage` works like anywhere else.

**Or enable `nix-ld`**, which also covers the extracted binary and the game runtime the editor starts for Play:

```nix
programs.nix-ld.enable = true;
programs.nix-ld.libraries = with pkgs; [
  vulkan-loader wayland libxkbcommon
  xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr
  alsa-lib udev fontconfig
];
```

Rebuild, then log out and back in so the session picks up the new environment.

Whichever you use, Vulkan needs graphics enabled:

```nix
hardware.graphics.enable = true;   # hardware.opengl.enable on NixOS 24.05 and earlier
```

## Alpine and other musl distros

Renzora needs glibc, and so do the GPU drivers it loads, so it cannot run on musl directly. Run it in a glibc container with [Distrobox](https://distrobox.it), which shares your display, GPU and home folder:

```sh
distrobox create --name renzora --image ubuntu:24.04
distrobox enter renzora
sudo apt install libvulkan1 mesa-vulkan-drivers libxkbcommon0 libxkbcommon-x11-0 libasound2 libfontconfig1
./Renzora*.AppImage --appimage-extract-and-run
```

The same works on any distro whose glibc is older than 2.28.

## If it does not start

Renzora checks the system before it starts and says what is wrong. It reports:

- **glibc too old.** The distro is older than the table above. Upgrade it, or use [Distrobox](#alpine-and-other-musl-distros).
- **Not a glibc system.** A musl distro; see [above](#alpine-and-other-musl-distros).
- **Missing libraries.** The message names them; install them with the line for your distro.
- **No Vulkan.** Install the Vulkan loader and your GPU's driver.
- **No display.** Neither a Wayland nor an X11 session is running. A game can still run headless with `--server`.

If the AppImage reports that **FUSE** is missing, start it with `--appimage-extract-and-run`, or install your distro's `fuse3` package.

Renzora uses Wayland when `WAYLAND_DISPLAY` is set and falls back to X11 otherwise.

If the check is wrong about your system, set `RENZORA_SKIP_PREFLIGHT=1` to start anyway.

## Games you export

A game exported with **Binary + .rpak** uses the editor's prebuilt runtime and runs on the same systems listed above. A **Lean single binary** is compiled on your machine, so it needs at least your machine's glibc; the export log warns when that is newer than 2.28. See [Export: Linux](/docs/r1-alpha8/exporting/linux).
