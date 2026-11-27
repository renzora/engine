# v6.0.0

* Bump DLSS SDK to v310.9.1
* Add support for aarch64 on Linux and Windows
* Add `transparency_overlay`, `color_before_transparency`, `depth_of_field_guide`, `responsivity_mask`, `alpha`, and `dlss_output_alpha` to `DlssRayReconstructionRenderParameters`
* Remove `bias` from `DlssRayReconstructionRenderParameters`
* `DlssRayReconstruction::new` now returns an error if `DlssFeatureFlags::AutoExposure` is set
* Fix dlss_wgpu incorrectly turning errors into Result::Ok(())
* Add DlssError::Unknown

# v5.0.0

* Upgrade to wgpu 30
* Bump DLSS SDK to v310.7.0

# v4.0.0
* Remove glam dependency
* `request_device` now accepts an `Option<Limits>` since wgpu's `open_with_callback` now requires it. The `Option<Limits>` will be used if provided, otherwise the value will fall back to `adapter.limits()`.
* Upgrade to wgpu 29
* Bump DLSS SDK to v310.5.3

# v3.0.0
* Upgrade to wgpu 28
* Bump DLSS SDK to v310.5.0
* DlssSuperResolution::render and DlssRayReconstruction::render now return a CommandBuffer that you must submit to a queue in a specific way. Read the documentation for more info.

# v2.0.0
* Upgrade to wgpu 27

# v1.0.1
* Lookup $DLSS_SDK at runtime, instead of compiling it into the binary for finding DLSS DLL locations
* Fix README instructions to mention that you need both the DLSS and DLSS-RR DLLs to use DLSS-RR
* Bump DLSS SDK to v310.4.0

# v1.0.0
* Initial release
