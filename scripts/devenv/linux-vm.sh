#!/bin/bash
# One-time setup inside a Linux test VM (Ubuntu or Debian family) so it can run
# the host checkout's Linux build straight from the share.
#
# After this, the host's checkout is at ~/engine, and the Linux editor starts with
#
#     ~/renzora-editor
#
# Rebuild on the host and the VM runs the new build next time; nothing is copied.
#
# Run it in the VM's terminal:
#
#     curl -fsSL https://raw.githubusercontent.com/renzora/engine/main/scripts/devenv/linux-vm.sh | bash
#
# It asks for the host's name (host-setup.ps1 prints it, for example
# DESKTOP-123ABC.mshome.net) and the password of the `engine` account that
# script created. RENZORA_HOST=<name> skips the first question.
set -euo pipefail

HOST="${RENZORA_HOST:-}"
SHARE="${RENZORA_SHARE:-engine}"
ACCOUNT="${RENZORA_ACCOUNT:-engine}"
MOUNT=/mnt/engine

if [ -z "$HOST" ]; then
    read -r -p "Windows host name (as shown by host-setup.ps1, e.g. DESKTOP-123ABC.mshome.net): " HOST </dev/tty
fi
read -r -s -p "Password for '$ACCOUNT' on $HOST: " PASS </dev/tty
echo

# cifs-utils mounts the share. The rest are the libraries the editor loads: a
# Hyper-V VM has no GPU, so Vulkan comes from Mesa's software driver (lavapipe),
# which is slow but enough to see that a build starts and works.
echo "Installing the share client and the editor's runtime libraries..."
sudo apt-get update -qq
sudo apt-get install -y -qq cifs-utils libvulkan1 mesa-vulkan-drivers \
    libxkbcommon0 libxkbcommon-x11-0 libasound2t64 libudev1 libfontconfig1 \
    2>/dev/null \
  || sudo apt-get install -y -qq cifs-utils libvulkan1 mesa-vulkan-drivers \
    libxkbcommon0 libxkbcommon-x11-0 libasound2 libudev1 libfontconfig1

# The password lives in a root-only file, not in /etc/fstab, which every user can read.
CRED=/etc/renzora-engine.cred
printf 'username=%s\npassword=%s\n' "$ACCOUNT" "$PASS" | sudo tee "$CRED" >/dev/null
sudo chmod 600 "$CRED"

# Owned by you and executable, because Windows' file system has no Unix modes to
# carry: without file_mode the binaries would not be runnable from the share.
# `nofail` so the VM still boots when the host is off; `_netdev` so it waits for
# the network.
OPTS="credentials=$CRED,uid=$(id -u),gid=$(id -g),file_mode=0755,dir_mode=0755,vers=3.1.1,_netdev,nofail"
LINE="//$HOST/$SHARE $MOUNT cifs $OPTS 0 0"
sudo mkdir -p "$MOUNT"
if grep -q " $MOUNT cifs " /etc/fstab; then
    sudo sed -i "\| $MOUNT cifs |d" /etc/fstab
fi
echo "$LINE" | sudo tee -a /etc/fstab >/dev/null
sudo systemctl daemon-reload
sudo umount "$MOUNT" 2>/dev/null || true
sudo mount "$MOUNT"

ln -sfn "$MOUNT" "$HOME/engine"

# A launcher that runs the AppDir's AppRun straight off the share. Not the
# .AppImage: that mounts itself with FUSE, which a network share and a fresh VM
# do not always allow. AppRun is what the AppImage would have run anyway, with
# the same Linux support check in front of the editor.
LAUNCHER="$HOME/renzora-editor"
cat > "$LAUNCHER" <<'EOF'
#!/bin/sh
# Starts the Linux editor from the host's checkout. Arguments pass through
# (for example --server, or --no-editor to run as the game).
arch=x64; [ "$(uname -m)" = "aarch64" ] && arch=arm64
dir="$HOME/engine/dist/linux-$arch"
apprun="$dir/Renzora Engine.AppDir/AppRun"
if [ -x "$apprun" ]; then exec "$apprun" "$@"; fi
image=$(ls "$dir"/*.AppImage 2>/dev/null | head -n1)
if [ -n "$image" ]; then exec "$image" --appimage-extract-and-run "$@"; fi
echo "No Linux build in $dir yet. Build one on the host first." >&2
exit 1
EOF
chmod +x "$LAUNCHER"

echo
echo "Mounted //$HOST/$SHARE at $MOUNT (also ~/engine). It reconnects at every boot."
echo "Start the Linux editor with:  ~/renzora-editor"
