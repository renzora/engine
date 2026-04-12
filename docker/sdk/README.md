Place `MacOSX26.2.sdk.tar.bz2` here.

Generate from your Xcode install:

```
git clone --depth 1 https://github.com/tpoechtrager/osxcross /tmp/osxcross
cd /tmp/osxcross
XCODEDIR=/Applications/Xcode.app/Contents/Developer ./tools/gen_sdk_package.sh
mv MacOSX26.2.sdk.tar.bz2 /path/to/repo/docker/sdk/
```

Deployment target is set to 14.0 in the Dockerfile, so binaries built
against this SDK still run on macOS 14+.
