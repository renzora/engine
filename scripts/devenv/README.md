# Testing every platform from one Windows PC

One checkout on a Windows PC builds the engine for Windows, Linux and macOS, and
Hyper-V machines run each build from that same folder. A fix made on the host is
in every OS's next build, and every VM runs the new build without copying
anything.

```
C:\...\engine                      (your checkout, shared as \\<pc>.mshome.net\engine)
  dist\windows-x64   ── Windows Sandbox   (fresh Windows every launch, real GPU)
  dist\linux-x64     ── Ubuntu VM         (Hyper-V, software rendering)
  dist\macos-x64     ── macOS VM          (Hyper-V, optional, see the end)
```

This is for checking that each editor **loads and runs**. Editors cross-built
for another OS have no working plugin SDK, so native plugins and Rust scripts
do not compile in them; the CI release lanes build every platform natively and
test that part.

## What you need

| | |
|---|---|
| Windows 11 **Pro** | Hyper-V and Windows Sandbox are not in Home |
| Hyper-V | `Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All` as Administrator, then reboot |
| [Docker Desktop](https://www.docker.com/products/docker-desktop/) | builds Linux, macOS and Windows in containers |
| Rust and the `renzora` CLI | [rustup.rs](https://rustup.rs), then `cargo install renzora` |
| 32 GB of RAM | the build's Docker VM alone takes up to half |
| 200 GB+ free | each platform keeps its own build cache |

**Put Docker's disk on a drive with room.** Its build caches go into one virtual
disk, which starts on `C:`. Move it before the first build: *Docker Desktop >
Settings > Resources > Advanced > Disk image location*. A disk that fills
mid-build does not fail cleanly; it turns into compile errors in crates nobody
touched.

## 1. Share the checkout (once)

In PowerShell opened with **Run as administrator**, from the checkout:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\devenv\host-setup.ps1
```

It asks you to choose a password, then:

- creates a local account, `engine`, for the VMs to sign in to the share with
  (hidden from the sign-in screen, with access to this checkout only), so your
  own Windows password never goes into a VM;
- shares the checkout as `\\<pc>.mshome.net\engine`;
- allows file sharing on the Hyper-V Default Switch only, a network inside this
  PC that nothing on your LAN can reach;
- turns on Windows Sandbox. **Reboot** once before using it.

It prints the share's address at the end. Run it again any time; it skips what
is already done.

## 2. Build

```powershell
cargo renzora dist                       # Windows, natively: dist\windows-x64
renzora build linux macos                # Linux and macOS, in Docker
```

The first Docker build compiles everything inside each container and takes a
few hours; later ones are incremental.

`renzora build windows` builds Windows in Docker too, but it writes the same
`dist\windows-x64` your everyday editor runs from, and that build has no
working plugin SDK. `cargo renzora dist` puts the native one back in seconds.

## 3. Test on Windows: Windows Sandbox

```powershell
powershell -ExecutionPolicy Bypass -File scripts\devenv\windows-sandbox.ps1
```

Opens a clean Windows that is wiped when you close it, with `dist\windows-x64`
mapped in, and starts the editor. Every launch is a machine with nothing
installed, which is what a user downloading the editor has. The sandbox shares
your GPU, so the editor renders for real.

Add `-Arguments '--no-editor'` to start the game runtime instead.

## 4. Test on Linux: an Ubuntu VM

1. In **Hyper-V Manager > Quick Create**, choose Ubuntu and create it (or
   install any Ubuntu or Debian ISO). Under the VM's **Settings > Memory**, turn
   on **Dynamic Memory** (start 2 GB, maximum 8 GB) so it does not hold 8 GB
   while it sits idle.
2. In the VM's terminal, with your PC's name from step 1:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/renzora/engine/main/scripts/devenv/linux-vm.sh | RENZORA_HOST=<pc>.mshome.net bash
   ```

   It asks for the `engine` password, mounts the share at `~/engine` (it
   reconnects at every boot), and installs the libraries the editor loads.
3. Start the Linux editor:

   ```bash
   ~/renzora-editor
   ```

A Hyper-V VM has no GPU, so Vulkan comes from Mesa's software driver: slow, but
enough to see that a build starts and works. Arguments pass through, so
`~/renzora-editor --no-editor` runs the game and `--server` the headless server.

## 5. CI checks in Docker

```powershell
renzora check          # cargo check, as CI runs it
renzora test           # the test suite, as CI runs it
```

## Memory

Everything here shares one PC's RAM. On 32 GB:

| | Takes |
|---|---|
| Docker during a build | up to 16 GB (half your RAM) |
| Ubuntu VM, dynamic | 2 GB idle, up to 8 GB |
| macOS VM | a fixed 6 to 8 GB (macOS cannot use dynamic memory) |
| Windows Sandbox | up to 8 GB while open |

So run one build at a time, and start the macOS VM when no build is running. A
VM that will not start with "Not enough memory in the system" is this; close
something or lower the VM's memory. To keep Docker smaller permanently, add to
`%USERPROFILE%\.wslconfig` and restart Docker:

```ini
[wsl2]
memory=12GB
```

## Troubleshooting

| Problem | Fix |
|---|---|
| The VM cannot reach `\\<pc>.mshome.net\engine` | The VM must be on the **Default Switch**. Check the name with `ping <pc>.mshome.net` in the VM. |
| "Permission denied" or a password error in the VM | Use the `engine` account and the password from step 1, not your own. Reset it with `net user engine *` as Administrator, then run `linux-vm.sh` again. |
| `Unable to find a GPU` in the Ubuntu VM | Mesa's Vulkan driver is missing: `sudo apt install mesa-vulkan-drivers`. |
| `windows-sandbox.ps1` says Sandbox is not on | Reboot after step 1. |

## Optional: macOS

> Apple's licence only permits macOS virtual machines on Apple hardware.

This uses [OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V) to run macOS
15 Sequoia (Intel, `x64`) under Hyper-V. A Hyper-V VM has no Metal, so the
editor cannot draw anything in it; what it can check is that the macOS build
loads and the headless server (`--server`) runs. Building and testing both Mac
architectures for real is the CI's macOS lanes.

1. **Prepare** (no Administrator; needs Python 3):

   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\devenv\macos\prepare.ps1 -Dir V:\VMs\osx-hyper-v
   ```

   Downloads OSX-Hyper-V's prebuilt OpenCore EFI, applies the AMD CPU patches
   if your CPU is AMD (for the VM's core count, 8 by default), and downloads
   the recovery image from Apple. Everything it fetches is pinned to a release
   or a commit; the project's own scripts instead run an unpinned bootstrap
   from GitHub as Administrator.
2. **Create the VM**, as Administrator:

   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\devenv\macos\create-vm.ps1 -Source V:\VMs\osx-hyper-v
   ```

3. **Install macOS.** Start the VM in Hyper-V Manager and open its console.
   - In OpenCore's menu, pick the recovery entry.
   - **Disk Utility > View > Show All Devices**, select **Msft Virtual Disk**,
     and **Erase** it as **APFS** with **GUID Partition Map**.
   - Quit Disk Utility, choose **Reinstall macOS**, and pick that disk.
   - It takes about an hour and restarts several times; each time, pick the
     disk's entry in OpenCore's menu.

   If it hangs early in boot on an AMD CPU, recreate the VM with `-Cores 6` and
   run `prepare.ps1 -Cores 6`; the core count is baked into the AMD patches.
4. **Reach the share** in macOS: **Finder > Go > Connect to Server**,
   `smb://<pc>.mshome.net/engine`, signing in as `engine`. The build is under
   `dist/macos-x64`.
