SDKs required for the Docker cross-compile builder.

# macOS — `MacOSX26.1.sdk.tar.xz`

Required for macOS x86_64 + arm64 builds.

```
git clone --depth 1 https://github.com/tpoechtrager/osxcross /tmp/osxcross
cd /tmp/osxcross
XCODEDIR=/Applications/Xcode.app/Contents/Developer ./tools/gen_sdk_package.sh
mv MacOSX26.1.sdk.tar.xz /path/to/repo/docker/sdk/
```

Deployment target is set to 11.0 in the Dockerfile, so binaries built
against this SDK still run on macOS 11+ (covers all Apple Silicon Macs
and Intel Macs running Big Sur or later).

# iOS — `iPhoneOS15.5.sdk.tar.xz`

Required for iOS arm64 builds. Two ways to obtain it:

**Option A: Download a community build** (fastest, doesn't need a Mac):
- https://github.com/growtopiajaw/iPhoneOS-SDK/releases
- Grab `iPhoneOS15.5.sdk.tar.xz` (or any version ≥ deployment target).
- These repos redistribute Apple-licensed content. The clean path is
  generating from your own Xcode (Option B); the SDKs are at-your-own-risk.

**Option B: Generate from Xcode** (license-clean):
```
git clone --depth 1 https://github.com/tpoechtrager/osxcross /tmp/osxcross
cd /tmp/osxcross
XCODEDIR=/Applications/Xcode.app/Contents/Developer ./tools/gen_sdk_package_p.sh iPhoneOS
mv iPhoneOS*.sdk.tar.xz /path/to/repo/docker/sdk/iPhoneOS15.5.sdk.tar.xz
```

If your iOS SDK version is different (e.g. 18.x from current Xcode), update
`OSXCROSS_IOS_SDK` in the Dockerfile to match the filename.

Deployment target is set to 14.0 in the Dockerfile (covers iPhone 6s and
later — every device that supports Metal API features used by Bevy/wgpu).
Newer SDK + older deployment target is fine.
