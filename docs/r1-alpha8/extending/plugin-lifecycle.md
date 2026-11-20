# From Plugin to Marketplace

How a [native plugin](./native-plugins.md) goes from an idea to something
anyone can install, and what happens to it with every engine build after that.

## 1. Make it

1. **Scaffold.** `renzora add <name>` writes the skeleton into the editor's
   `plugins/` directory: a `Cargo.toml`, and a `src/lib.rs` with one
   `renzora::plugin!(MyPlugin, Editor)` (or `Runtime`) line. See
   [Native Plugins](./native-plugins.md) for what a plugin can reach.
2. **Develop.** Launch the editor. Its setup window compiles the plugin against
   the editor's own SDK, and it loads. Edit and relaunch: it recompiles whenever
   its source or the engine changes. A build that fails is written to
   `plugins/<name>/build/build.log` and named under **Settings ▸ Editor ▸
   Plugins**. Never run `cargo build` inside the plugin's folder; see
   [Building](./native-plugins.md#building).
3. **Describe it for the marketplace,** in `Cargo.toml`:

   ```toml
   [package]
   name = "my_plugin"
   version = "1.0.0"

   [package.metadata.renzora]
   marketplace_id = "my_plugin"   # your listing's permanent handle, first come first served
   category = "plugins"
   tags = ["rendering"]
   price_credits = 0
   min_engine_version = "r1-alpha8"
   ```

   The `marketplace_id` matters beyond the listing: it is what a prebuilt
   binary of your plugin is filed under and matched by, so a listing without
   one never gets one.

## 2. Publish it

1. **Become a creator** on renzora.com and mint an API token at
   renzora.com/developers.
2. **Publish:** `renzora publish` from the plugin's folder (or the editor's
   Publish panel, or the upload wizard on the website). It uploads the source
   and creates a **release** for the plugin's version, which states the oldest
   engine it runs on. See [Publishing](../marketplace/publishing.md) and its
   [engine compatibility](../marketplace/publishing.md#engine-compatibility)
   rules.
3. **It is live.** Editors whose engine is new enough see it, and installing it
   installs its source.

To update, bump `version` and publish again. Users who have it are told, and
users on an older engine keep being offered the newest release that still runs
on theirs.

### Official plugins

Plugins in [renzora/plugins](https://github.com/renzora/plugins) are published
by that repository instead: a pull request runs a **Dry run** that checks the
publish without uploading, and merging to `main` publishes every plugin that
changed **and** bumped its version. A change without a version bump publishes
nothing, so the version bump is the release.

## 3. Compiled against every official build

You never compile the binary people run, and neither does anyone's own engine,
which could be a custom build of anything. Renzora's CI compiles your uploaded
source against each **official** build:

- **Every nightly and every stable release** compiles every marketplace plugin,
  each against that build's SDK, on Windows, macOS and Linux.
- **When you publish a release,** your plugin is compiled straight away against
  the latest nightly and the latest stable build, so you find out within the
  hour rather than at the next nightly.

What each build produces:

- **It compiles:** a signed [prebuilt binary](./prebuilt-plugins.md) for that
  build and platform. Anyone on that build installs it with nothing compiled
  and no Rust toolchain; anyone else (an engine built from source) compiles
  your source as before.
- **It does not compile:** a recorded failure. Your listing on renzora.com
  shows which platforms it fails on, and as its creator you see the compiler's
  output there. Someone installing it on that build gets no prebuilt and
  compiles your source, which fails the same way.

The usual reason a working plugin stops compiling is the engine changing an API
under it. Check the failing build's output, fix, bump the version and publish:
the new release is compiled against the latest builds as soon as it lands.

### How your code is treated

Compiling a crate runs its code (`build.rs`, proc macros, and those of its
dependencies), so third-party plugins compile in isolation: one fresh machine
per plugin per platform, holding no secret and unable to change anything, with
signing done separately, by a job that compiles nothing of yours and signs only
facts it established itself (your listing's id and version, the official
build, and a hash of the result). A binary's signature therefore says "built by
Renzora's CI from the source the creator uploaded, against this official
build", which is the same trust as compiling the plugin yourself. It is not a
code review: a plugin is native code with the editor's full privileges either
way, so install plugins from creators you trust.
