//! # dlss_wgpu
//!
//! This crate provides Rust bindings for integrating NVIDIA DLSS (Deep Learning Super Sampling) with the `wgpu` graphics API.
//!
//! ## Setup
//! See <https://github.com/bevyengine/dlss_wgpu/blob/main/README.md> for setup instructions.
//!
//! This crate only works with wgpu's Vulkan backend. Other backends are not supported.
//!
//! For further info on how to integrate DLSS into your application, read `$DLSS_SDK/doc/DLSS_Programming_Guide_Release.pdf`.
//!
//! ## API Usage
//! ```compile_fail
//! use dlss_wgpu::{FeatureSupport, DlssSdk, DlssPerfQualityMode, DlssFeatureFlags};
//! use dlss_wgpu::super_resolution::{DlssSuperResolution, DlssSuperResolutionRenderParameters};
//!
//! let project_id = Uuid::parse_str("...").unwrap();
//! let mut feature_support = FeatureSupport::default();
//!
//! // Initialize wgpu
//! let instance = dlss_wgpu::create_instance(project_id, &instance_descriptor, &mut feature_support).unwrap();
//! let adapter = instance.request_adapter(&adapter_options).await.unwrap();
//! let (device, queue) = dlss_wgpu::request_device(project_id, &adapter, &device_descriptor, &mut feature_support, None).unwrap();
//!
//! // Check for feature support, if false don't create DLSS resources
//! println!("DLSS supported: {}", feature_support.super_resolution_supported);
//!
//! // Create the SDK once per application
//! let sdk = DlssSdk::new(project_id, device).expect("Failed to create DlssSdk");
//!
//! // Create a DLSS context once per camera or when DLSS settings change
//! let mut context = DlssSuperResolution::new(
//!     camera.output_resolution,
//!     DlssPerfQualityMode::Auto,
//!     DlssFeatureFlags::empty(),
//!     Arc::clone(&sdk),
//!     &device,
//!     &queue,
//! )
//! .expect("Failed to create DlssSuperResolution");
//!
//! // Setup camera settings
//! camera.view_size = context.render_resolution();
//! camera.subpixel_jitter = context.suggested_jitter(frame_number, camera.view_size);
//! camera.mip_bias = context.suggested_mip_bias(camera.view_size);
//!
//! // Encode DLSS render commands
//! let render_parameters = DlssSuperResolutionRenderParameters { ... };
//! let dlss_command_buffer = context.render(render_parameters, &mut command_encoder, &adapter)
//!     .expect("Failed to render DLSS");
//!
//! // Submit render commands
//! queue.submit([command_encoder.finsh(), dlss_command_buffer]);
//! ```

#[cfg(not(feature = "mock"))]
mod feature_info;
#[cfg(not(feature = "mock"))]
mod initialization;
#[cfg(not(feature = "mock"))]
mod nvsdk_ngx;
#[cfg(not(feature = "mock"))]
mod sdk;

/// DLSS Ray Reconstruction.
#[cfg(not(feature = "mock"))]
pub mod ray_reconstruction;
/// DLSS Super Resolution.
#[cfg(not(feature = "mock"))]
pub mod super_resolution;

#[cfg(not(feature = "mock"))]
pub use initialization::{
    FeatureSupport, InitializationError, create_instance, register_device_extensions,
    register_instance_extensions, request_device,
};
#[cfg(not(feature = "mock"))]
pub use nvsdk_ngx::{DlssError, DlssFeatureFlags, DlssPerfQualityMode};
#[cfg(not(feature = "mock"))]
pub use sdk::DlssSdk;
