# Prebuilt Plugins

A [native plugin](./native-plugins.md) ships as source and is compiled on the
machine that installs it, against that engine's SDK. That needs the SDK unpacked
(about 1.9 GB) and a Rust toolchain. On an official nightly or release it does
not have to: the engine's CI compiles every plugin in
[renzora/plugins](https://github.com/renzora/plugins) for that exact build, on
Windows, macOS and Linux, and the marketplace serves the result.

## Installing

Nothing to choose. **Install** in the editor's marketplace (or on the start
screen) installs the plugin's source, then:

1. asks the marketplace for a binary built for **this exact engine build** and
   this platform;
2. checks it (see [Security](#security));
3. puts it where a source build would have left its library, so it loads on the
   next start with nothing compiled.

If there is no binary for this build, or it fails any check, the plugin is
compiled from source as before, and the install message says why. Tick **Build
from source** in the install dialog to compile it yourself even when a binary
exists.

The plugin's source is always installed too: the [lean exporter](../exporting/overview.md)
compiles plugins into a game from source, and it is what a plugin is rebuilt
from on an engine build no binary exists for.

### Which builds get binaries

The engine's **build stamp** decides, not its version. The stamp is a content
hash of the engine images a plugin links against, so every copy of one nightly
has the same stamp and every build has a different one. An official build writes
its stamp to `plugins/engine-stamp.txt` beside the executable, so it is known
before the SDK is unpacked.

A build you made yourself with `cargo renzora` has a stamp no binary carries, so
its editor finds none and compiles from source without being told to. That is
correct: a prebuilt only runs on the build it was compiled against.

Every marketplace plugin with a `marketplace_id` is prebuilt: the ones in
renzora/plugins by the engine's own build lanes, and third-party ones by the
`third-party-plugins` workflow, which compiles their uploaded source in
isolation against the same official SDKs. See
[From Plugin to Marketplace](./plugin-lifecycle.md#3-compiled-against-every-official-build)
for when that happens and what a creator sees when it fails.

### After an engine update

Every nightly is a new build, so every installed plugin is out of date with it.
On the first launch the setup pass asks the marketplace for each marketplace
plugin's binary for the new build and installs it; only a plugin with no binary
is compiled. A plugin is asked about once per build: if it has none, it is not
asked again until the engine changes, so a missing binary never reopens the
setup window on every launch.

## Security

A prebuilt is a native library mapped into the editor's own process, so it is
accepted only when all of these hold:

| Check | What it stops |
|---|---|
| CI's **Ed25519 signature** over the binary's statement (plugin id, version, platform, engine stamp, SHA-256, size) verifies against a key compiled into the engine | A binary CI did not build. The marketplace never has the private key, so even a compromised marketplace cannot make an editor accept a binary of its own. |
| The statement names the plugin and **version whose source was just installed** | A validly signed binary of a different plugin, or an older version, served in its place |
| The statement is for **this platform** and **this engine stamp** | A binary for another OS or another build |
| The downloaded bytes have the statement's **size** and **SHA-256** | Corruption or tampering in transit or in storage |
| The library itself **records this engine build** (the ABI record `renzora::plugin!` writes, which the loader checks too) | A mislabelled binary |

Any failure means the binary is not written and the plugin compiles from source.

The trusted public keys are in `crates/renzora_plugin_build/prebuilt_public_keys.txt`.
With none listed (a fork, or before a key is set up) every prebuilt is refused,
which is how installs worked before prebuilts existed.

## Setting up signing (maintainers)

Once, to create the key pair. The secret key goes straight into the CI secret
and is never printed or written to disk; the public key is appended to the keys
file, to commit:

```sh
cargo run --profile dist -p renzora_plugin_build --example prebuilt_keygen -- \
    --public-out crates/renzora_plugin_build/prebuilt_public_keys.txt \
  | gh secret set PREBUILT_SIGNING_KEY --repo renzora/engine
```

CI also needs `RENZORA_MARKETPLACE_TOKEN` on the engine repository: the same
kind of renzora.com API token (`rz_…`) renzora/plugins publishes with, from an
**admin** account, which the upload endpoint requires. GitHub secrets are per
repository, so the engine needs its own copy even when the plugins repository
already has one.

To rotate, run the same command again (it appends), ship a release with both
keys, and only then remove the old one, so binaries signed with it stay
installable until they are replaced.

## How CI builds them

On a nightly or release, each desktop lane runs the plugin check it already ran
with `--emit`: every plugin that builds against that lane's SDK and carries its
build stamp is written out with a signed `prebuilt.json`. Once the release is
published, the `prebuilts` job uploads them to `POST /api/prebuilt-plugins/upload`.
The marketplace files each under its listing (by `marketplace_id`), platform and
engine stamp, and stores the bytes once per SHA-256.

Third-party plugins go through `.github/workflows/third-party-plugins.yml`, run
after every published nightly and release (from `build-engine.yml`), when a
creator publishes a release (`plugin-release.yml`, dispatched by the website),
or by hand with a release tag. Its jobs keep untrusted code away from anything
that matters:

| Job | Secrets | Runs third-party code | Does |
|---|---|---|---|
| `list` | marketplace token | no | Asks `GET /build-list` which release of each plugin the engine version installs, leaves out renzora/plugins, fetches each source with `GET /source/...` |
| `sdk` | none | no | Takes the SDK out of the official release asset and builds the `unpack_sdk` and `check_plugins` tools, once per platform |
| `build` | none, `permissions: {}` | **yes** | One fresh machine per plugin per platform: unpack the SDK, compile, keep the library and the compiler's output |
| `publish` | signing key, marketplace token | no | `sign_prebuilt` signs each library from the listed id and version, the job's platform, the SDK's stamp and a hash it measures (never from anything a build job wrote), uploads it, and records every result with `POST /build-result` |

The website asks for a build when a plugin release is published only if it has
`ENGINE_DISPATCH_TOKEN` (a GitHub token allowed to dispatch to the engine
repository) in its environment; without it, new releases wait for the nightly.

To build and sign them locally against your own SDK, for testing:

```sh
PREBUILT_SIGNING_KEY=<hex secret> cargo run --profile dist -p renzora_plugin_build \
    --example check_plugins -- --sdk dist/windows-x64/sdk --plugins ../plugins --emit out/
```

## The marketplace API

| Route | Use |
|---|---|
| `GET /api/prebuilt-plugins/<asset id>?engine=<stamp>&platform=<platform>` | The binary for this exact build, or 404 |
| `GET /api/prebuilt-plugins/<asset id>/latest` | The newest binary per platform, for the listing page |
| `GET /api/prebuilt-plugins/<asset id>/file/<id>` | The bytes, counted as a download |
| `POST /api/prebuilt-plugins/upload` | CI's upload; admin token, multipart `statement`, `engine_tag`, `engine_version`, `library` |

Platforms use the engine's `dist/` names: `windows-x64`, `windows-arm64`,
`macos-x64`, `macos-arm64`, `linux-x64`, `linux-arm64`.

A plugin's listing page on renzora.com shows its prebuilt binaries under the
download button, leading with the visitor's own platform.
