SDKs for the Docker cross-compile builder.

The macOS and iOS SDKs are **downloaded automatically at image-build time** by
`docker/engine-builder/Dockerfile`, so you normally don't need to place anything
here. The download URLs and versions live in the Dockerfile:

```
ENV OSXCROSS_SDK=MacOSX26.1.sdk
ENV OSXCROSS_IOS_SDK=iPhoneOS15.5.sdk
ENV OSXCROSS_SDK_URL=https://github.com/joseluisq/macosx-sdks/releases/download/26.1/MacOSX26.1.sdk.tar.xz
ENV OSXCROSS_IOS_SDK_URL=https://github.com/growtopiajaw/iPhoneOS-SDK/releases/download/v1.0/iPhoneOS15.5.sdk.tar.xz
```

These URLs point to community mirrors that redistribute Apple-licensed SDK
content; use at your own discretion. To pin a different version, update the
`*_URL` and matching `OSXCROSS_SDK` / `OSXCROSS_IOS_SDK` names together.

Deployment targets (set in the Dockerfile): macOS 11.0 (covers all Apple
Silicon Macs and Intel Macs on Big Sur+) and iOS 14.0 (iPhone 6s and later —
every device with the Metal features Bevy/wgpu uses). A newer SDK with an older
deployment target is fine.

## Generating license-clean SDKs yourself (optional)

If you'd rather not use the community mirrors, generate the tarballs from your
own Xcode on a Mac and host them somewhere the build can reach (or revert the
Dockerfile to `COPY` them from this directory):

macOS:
```
git clone --depth 1 https://github.com/tpoechtrager/osxcross /tmp/osxcross
cd /tmp/osxcross
XCODEDIR=/Applications/Xcode.app/Contents/Developer ./tools/gen_sdk_package.sh
```

iOS:
```
git clone --depth 1 https://github.com/tpoechtrager/osxcross /tmp/osxcross
cd /tmp/osxcross
XCODEDIR=/Applications/Xcode.app/Contents/Developer ./tools/gen_sdk_package_p.sh iPhoneOS
```
