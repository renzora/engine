# Installing Docker

Docker is how Renzora builds for platforms you are not sitting at: a macOS or Android build from a Windows machine, a Windows build from Linux. Each target needs its own compiler, linker and SDK, and the engine's toolchain images carry all of them, so the only thing your machine needs is Docker.

**You do not need Docker to use the editor**, to build the engine for your own machine (`cargo renzora`), or to export a game for your own platform or the web. Install it when an export or `renzora build` asks for it.

The images are a few gigabytes each, and the CLI pulls only the ones a command needs: `renzora build windows` downloads the Windows toolchain and nothing else.

## Windows

Install **Docker Desktop** with the WSL 2 backend, from [docker.com](https://www.docker.com/products/docker-desktop/) or with winget:

```powershell
winget install Docker.DockerDesktop
```

Restart when it asks, then start Docker Desktop once so it finishes setting up WSL 2.

If Docker Desktop stays on "Starting" forever, hardware virtualization is off. Turn it on in your BIOS/UEFI settings: **SVM Mode** on AMD boards, **Intel Virtualization Technology (VT-x)** on Intel ones.

## macOS

Install **Docker Desktop** from [docker.com](https://www.docker.com/products/docker-desktop/) or with Homebrew:

```sh
brew install --cask docker
```

Open it once from Applications to finish setup. It runs on both Intel and Apple Silicon; the toolchain images are built for both.

## Linux

Install Docker Engine from your distro, start it, and add yourself to the `docker` group so it runs without `sudo`.

**Ubuntu, Debian, Linux Mint, Pop!\_OS, elementary OS, Zorin OS**

```sh
sudo apt install docker.io
```

**Fedora**

```sh
sudo dnf install moby-engine
```

**RHEL, Rocky Linux, AlmaLinux**

These ship Podman instead of Docker. Add Docker's own repository and install from it:

```sh
sudo dnf config-manager --add-repo https://download.docker.com/linux/rhel/docker-ce.repo
sudo dnf install docker-ce docker-ce-cli containerd.io
```

**Arch, Manjaro, EndeavourOS, CachyOS**

```sh
sudo pacman -S docker
```

**openSUSE Tumbleweed**

```sh
sudo zypper install docker
```

**Fedora Silverblue, Kinoite, Bazzite**

```sh
rpm-ostree install moby-engine
systemctl reboot
```

Then, on all of the above:

```sh
sudo systemctl enable --now docker
sudo usermod -aG docker "$USER"
```

Log out and back in so the group change takes effect.

**NixOS**

In `configuration.nix`, replacing `you` with your user name:

```nix
virtualisation.docker.enable = true;
users.users.you.extraGroups = [ "docker" ];
```

Run `sudo nixos-rebuild switch`, then log out and back in.

**Steam Deck (SteamOS)** has a read-only system partition, so Docker cannot be installed in the normal way. Cross-compile from another machine instead.

## Check that it works

```sh
docker run --rm hello-world
```

It should print "Hello from Docker!". If it says permission denied on Linux, the group change has not taken effect yet: log out and back in.

## Set up the Renzora toolchain

Install the `renzora` CLI and let it fetch the toolchain for your machine:

```sh
cargo install renzora
renzora init
```

After that, `renzora build <platform>` cross-compiles (for example `renzora build windows macos`), and the editor's export dialog uses the same images for a lean export to another OS. See [Building from Source](/docs/r1-alpha8/setup/building-from-source#cross-compiling-for-other-platforms) for the commands, and [Cross-Platform Exporting](/docs/r1-alpha8/exporting/cross-platform) for what each platform produces.

One limit: the **Android** toolchain only runs on x86_64. On an ARM64 machine (Apple Silicon, ARM Linux) every other platform builds, but Android needs an x86_64 host.
