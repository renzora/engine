use crate::{DlssSdk, nvsdk_ngx::*};
use std::{
    iter, ptr,
    sync::{Arc, Mutex},
};
use wgpu::{
    Adapter, CommandBuffer, CommandEncoder, CommandEncoderDescriptor, Device, Queue, Texture,
    TextureTransition, TextureUses, TextureView, hal::api::Vulkan,
};

/// Camera-specific object for using DLSS Ray Reconstruction.
pub struct DlssRayReconstruction {
    upscaled_resolution: [u32; 2],
    render_resolution: [u32; 2],
    device: Device,
    sdk: Arc<Mutex<DlssSdk>>,
    feature: *mut NVSDK_NGX_Handle,
}

impl DlssRayReconstruction {
    /// Create a new [`DlssRayReconstruction`] object.
    ///
    /// This is an expensive operation. The resulting object should be cached, and only recreated when settings change.
    ///
    /// This should only be called if [`crate::FeatureSupport::ray_reconstruction_supported`] is true.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        upscaled_resolution: [u32; 2],
        perf_quality_mode: DlssPerfQualityMode,
        feature_flags: DlssFeatureFlags,
        roughness_mode: DlssRayReconstructionRoughnessMode,
        depth_mode: DlssRayReconstructionDepthMode,
        sdk: Arc<Mutex<DlssSdk>>,
        device: &Device,
        queue: &Queue,
    ) -> Result<Self, DlssError> {
        // Not supported by ray reconstruction
        if feature_flags.contains(DlssFeatureFlags::AutoExposure) {
            return Err(DlssError::UnsupportedParameter);
        }

        let locked_sdk = sdk.lock().unwrap();

        let perf_quality_value = perf_quality_mode.as_perf_quality_value(upscaled_resolution);

        let mut optimal_render_resolution = [0, 0];
        let mut min_render_resolution = [0, 0];
        let mut max_render_resolution = [0, 0];
        unsafe {
            let mut deprecated_sharpness = 0.0f32;
            check_ngx_result(NGX_DLSSD_GET_OPTIMAL_SETTINGS(
                locked_sdk.parameters,
                upscaled_resolution[0],
                upscaled_resolution[1],
                perf_quality_value,
                &mut optimal_render_resolution[0],
                &mut optimal_render_resolution[1],
                &mut max_render_resolution[0],
                &mut max_render_resolution[1],
                &mut min_render_resolution[0],
                &mut min_render_resolution[1],
                &mut deprecated_sharpness,
            ))?;
        }
        if perf_quality_mode == DlssPerfQualityMode::Dlaa {
            optimal_render_resolution = upscaled_resolution;
        }

        let mut create_params = NVSDK_NGX_DLSSD_Create_Params {
            InDenoiseMode: NVSDK_NGX_DLSS_Denoise_Mode_NVSDK_NGX_DLSS_Denoise_Mode_DLUnified,
            InRoughnessMode: match roughness_mode {
                DlssRayReconstructionRoughnessMode::Unpacked => {
                    NVSDK_NGX_DLSS_Roughness_Mode_NVSDK_NGX_DLSS_Roughness_Mode_Unpacked
                }
                DlssRayReconstructionRoughnessMode::Packed => {
                    NVSDK_NGX_DLSS_Roughness_Mode_NVSDK_NGX_DLSS_Roughness_Mode_Packed
                }
            },
            InUseHWDepth: match depth_mode {
                DlssRayReconstructionDepthMode::Linear => {
                    NVSDK_NGX_DLSS_Depth_Type_NVSDK_NGX_DLSS_Depth_Type_Linear
                }
                DlssRayReconstructionDepthMode::Hardware => {
                    NVSDK_NGX_DLSS_Depth_Type_NVSDK_NGX_DLSS_Depth_Type_HW
                }
            },
            InWidth: optimal_render_resolution[0],
            InHeight: optimal_render_resolution[1],
            InTargetWidth: upscaled_resolution[0],
            InTargetHeight: upscaled_resolution[1],
            InPerfQualityValue: perf_quality_value,
            InFeatureCreateFlags: feature_flags.as_flags(),
            InEnableOutputSubrects: feature_flags.contains(DlssFeatureFlags::OutputSubrect),
        };

        let mut command_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("dlss_ray_reconstruction_context_creation"),
        });

        let mut feature = ptr::null_mut();
        unsafe {
            let hal_device = device.as_hal::<Vulkan>().unwrap();
            command_encoder.as_hal_mut::<Vulkan, _, _>(|command_encoder| {
                check_ngx_result(NGX_VULKAN_CREATE_DLSSD_EXT1(
                    hal_device.raw_device().handle(),
                    command_encoder.unwrap().raw_handle(),
                    1,
                    1,
                    &mut feature,
                    locked_sdk.parameters,
                    &mut create_params,
                ))
            })?
        }

        queue.submit([command_encoder.finish()]);

        Ok(Self {
            upscaled_resolution,
            render_resolution: optimal_render_resolution,
            device: device.clone(),
            sdk: Arc::clone(&sdk),
            feature,
        })
    }

    /// Encode rendering commands for DLSS Ray Reconstruction.
    ///
    /// The resulting command buffer should be submitted to a [`Queue`] in the same submit as the finished `command_encoder`, ordered immediately afterwards.
    /// ```compile_fail
    /// let mut my_command_encoder = device.create_command_encoder(descriptor);
    /// let dlss_command_buffer = dlss.render(render_parameters, &mut my_command_encoder, adapter).unwrap();
    /// queue.submit([my_command_encoder.finish(), dlss_command_buffer]);
    /// ```
    ///
    /// Failing to follow these rules is undefined behavior.
    pub fn render(
        &mut self,
        render_parameters: DlssRayReconstructionRenderParameters,
        command_encoder: &mut CommandEncoder,
        adapter: &Adapter,
    ) -> Result<CommandBuffer, DlssError> {
        render_parameters.validate()?;

        let sdk = self.sdk.lock().unwrap();

        let partial_texture_size = render_parameters
            .partial_texture_size
            .unwrap_or(self.render_resolution);

        // NGX reads these through raw pointers during EvaluateFeature. The bindings
        // must stay alive until the evaluate call below.
        let mut diffuse_albedo = texture_to_ngx(render_parameters.diffuse_albedo, adapter);
        let mut specular_albedo = texture_to_ngx(render_parameters.specular_albedo, adapter);
        let mut normals = texture_to_ngx(render_parameters.normals, adapter);
        let mut roughness = render_parameters
            .roughness
            .map(|roughness| texture_to_ngx(roughness, adapter));
        let mut color = texture_to_ngx(render_parameters.color, adapter);
        let mut dlss_output = texture_to_ngx(render_parameters.dlss_output, adapter);
        let mut depth = texture_to_ngx(render_parameters.depth, adapter);
        let mut motion_vectors = texture_to_ngx(render_parameters.motion_vectors, adapter);
        let mut transparency_layer = None;
        let mut transparency_layer_opacity = None;
        match render_parameters.transparency_overlay {
            Some(DlssRayReconstructionTransparencyOverlay::Premultiplied(layer)) => {
                transparency_layer = Some(texture_to_ngx(layer, adapter));
            }
            Some(DlssRayReconstructionTransparencyOverlay::Separate { color, opacity }) => {
                transparency_layer = Some(texture_to_ngx(color, adapter));
                transparency_layer_opacity = Some(texture_to_ngx(opacity, adapter));
            }
            None => {}
        }
        let mut color_before_transparency = render_parameters
            .color_before_transparency
            .map(|color| texture_to_ngx(color, adapter));
        let mut depth_of_field_guide = render_parameters
            .depth_of_field_guide
            .map(|guide| texture_to_ngx(guide, adapter));
        let mut screen_space_subsurface_scattering_guide = render_parameters
            .screen_space_subsurface_scattering_guide
            .map(|guide| texture_to_ngx(guide, adapter));
        let mut responsivity_mask = render_parameters
            .responsivity_mask
            .map(|mask| texture_to_ngx(mask, adapter));
        let mut alpha = render_parameters
            .alpha
            .map(|alpha| texture_to_ngx(alpha, adapter));
        let mut dlss_output_alpha = render_parameters
            .dlss_output_alpha
            .map(|alpha| texture_to_ngx(alpha, adapter));
        let mut specular_motion_vectors = None;
        let mut specular_hit_distance = None;
        let mut world_to_view = None;
        let mut view_to_clip = None;
        match render_parameters.specular_guide {
            DlssRayReconstructionSpecularGuide::SpecularMotionVectors(motion_vectors) => {
                specular_motion_vectors = Some(texture_to_ngx(motion_vectors, adapter));
            }
            DlssRayReconstructionSpecularGuide::SpecularHitDistance {
                texture_view,
                world_to_view_rows_array,
                view_to_clip_rows_array,
            } => {
                specular_hit_distance = Some(texture_to_ngx(texture_view, adapter));
                world_to_view = Some(world_to_view_rows_array);
                view_to_clip = Some(view_to_clip_rows_array);
            }
        }

        let mut eval_params = NVSDK_NGX_VK_DLSSD_Eval_Params {
            pInResponsivityMask: responsivity_mask
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInDiffuseAlbedo: &mut diffuse_albedo,
            pInSpecularAlbedo: &mut specular_albedo,
            pInNormals: &mut normals,
            pInRoughness: roughness.as_mut().map_or(ptr::null_mut(), ptr::from_mut),
            pInColor: &mut color,
            pInAlpha: alpha.as_mut().map_or(ptr::null_mut(), ptr::from_mut),
            pInOutput: &mut dlss_output,
            pInOutputAlpha: dlss_output_alpha
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInDepth: &mut depth,
            pInMotionVectors: &mut motion_vectors,
            InJitterOffsetX: render_parameters.jitter_offset[0],
            InJitterOffsetY: render_parameters.jitter_offset[1],
            InRenderSubrectDimensions: NVSDK_NGX_Dimensions {
                Width: partial_texture_size[0],
                Height: partial_texture_size[1],
            },
            InReset: render_parameters.reset as _,
            InMVScaleX: render_parameters.motion_vector_scale.unwrap_or([1.0, 1.0])[0],
            InMVScaleY: render_parameters.motion_vector_scale.unwrap_or([1.0, 1.0])[1],
            pInTransparencyMask: ptr::null_mut(),
            pInExposureTexture: ptr::null_mut(),
            pInBiasCurrentColorMask: ptr::null_mut(),
            InAlphaSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InOutputAlphaSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InDiffuseAlbedoSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InSpecularAlbedoSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InNormalsSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InRoughnessSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InDepthSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InMVSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InTranslucencySubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InBiasCurrentColorSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InOutputSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InPreExposure: 0.0,
            InExposureScale: 0.0,
            InIndicatorInvertXAxis: 0,
            InIndicatorInvertYAxis: 0,
            InResponsivityMaskSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            pInReflectedAlbedo: ptr::null_mut(),
            pInColorBeforeParticles: ptr::null_mut(),
            pInColorAfterParticles: ptr::null_mut(),
            pInColorBeforeTransparency: color_before_transparency
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInColorAfterTransparency: ptr::null_mut(),
            pInColorBeforeFog: ptr::null_mut(),
            pInColorAfterFog: ptr::null_mut(),
            pInScreenSpaceSubsurfaceScatteringGuide: screen_space_subsurface_scattering_guide
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInColorBeforeScreenSpaceSubsurfaceScattering: ptr::null_mut(),
            pInColorAfterScreenSpaceSubsurfaceScattering: ptr::null_mut(),
            pInScreenSpaceRefractionGuide: ptr::null_mut(),
            pInColorBeforeScreenSpaceRefraction: ptr::null_mut(),
            pInColorAfterScreenSpaceRefraction: ptr::null_mut(),
            pInDepthOfFieldGuide: depth_of_field_guide
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInColorBeforeDepthOfField: ptr::null_mut(),
            pInColorAfterDepthOfField: ptr::null_mut(),
            pInDiffuseHitDistance: ptr::null_mut(),
            pInSpecularHitDistance: specular_hit_distance
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInDiffuseRayDirection: ptr::null_mut(),
            pInSpecularRayDirection: ptr::null_mut(),
            pInDiffuseRayDirectionHitDistance: ptr::null_mut(),
            pInSpecularRayDirectionHitDistance: ptr::null_mut(),
            InReflectedAlbedoSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorBeforeParticlesSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorAfterParticlesSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorBeforeTransparencySubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorAfterTransparencySubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorBeforeFogSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorAfterFogSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InScreenSpaceSubsurfaceScatteringGuideSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorBeforeScreenSpaceSubsurfaceScatteringSubrectBase: NVSDK_NGX_Coordinates {
                X: 0,
                Y: 0,
            },
            InColorAfterScreenSpaceSubsurfaceScatteringSubrectBase: NVSDK_NGX_Coordinates {
                X: 0,
                Y: 0,
            },
            InScreenSpaceRefractionGuideSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorBeforeScreenSpaceRefractionSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorAfterScreenSpaceRefractionSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InDepthOfFieldGuideSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorBeforeDepthOfFieldSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InColorAfterDepthOfFieldSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InDiffuseHitDistanceSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InSpecularHitDistanceSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InDiffuseRayDirectionSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InSpecularRayDirectionSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InDiffuseRayDirectionHitDistanceSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            InSpecularRayDirectionHitDistanceSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            pInWorldToViewMatrix: world_to_view
                .as_mut()
                .map_or(ptr::null_mut(), |matrix| ptr::from_mut(matrix).cast()),
            pInViewToClipMatrix: view_to_clip
                .as_mut()
                .map_or(ptr::null_mut(), |matrix| ptr::from_mut(matrix).cast()),
            GBufferSurface: NVSDK_NGX_VK_GBuffer {
                pInAttrib: [ptr::null_mut(); 17],
            },
            InToneMapperType: NVSDK_NGX_ToneMapperType_NVSDK_NGX_TONEMAPPER_STRING,
            pInMotionVectors3D: ptr::null_mut(),
            pInIsParticleMask: ptr::null_mut(),
            pInAnimatedTextureMask: ptr::null_mut(),
            pInDepthHighRes: ptr::null_mut(),
            pInPositionViewSpace: ptr::null_mut(),
            InFrameTimeDeltaInMsec: 0.0,
            pInRayTracingHitDistance: ptr::null_mut(),
            pInMotionVectorsReflections: specular_motion_vectors
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            pInTransparencyLayer: transparency_layer
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            InTransparencyLayerSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            pInTransparencyLayerOpacity: transparency_layer_opacity
                .as_mut()
                .map_or(ptr::null_mut(), ptr::from_mut),
            InTransparencyLayerOpacitySubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            pInTransparencyLayerMvecs: ptr::null_mut(),
            InTransparencyLayerMvecsSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
            pInDisocclusionMask: ptr::null_mut(),
            InDisocclusionMaskSubrectBase: NVSDK_NGX_Coordinates { X: 0, Y: 0 },
        };

        command_encoder.transition_resources(iter::empty(), render_parameters.barrier_list());

        let mut dlss_command_encoder =
            self.device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("dlss_ray_reconstruction"),
                });
        unsafe {
            dlss_command_encoder.as_hal_mut::<Vulkan, _, _>(|command_encoder| {
                check_ngx_result(NGX_VULKAN_EVALUATE_DLSSD_EXT(
                    command_encoder.unwrap().raw_handle(),
                    self.feature,
                    sdk.parameters,
                    &mut eval_params,
                ))
            })?;
        }
        Ok(dlss_command_encoder.finish())
    }

    /// Suggested subpixel camera jitter for a given frame.
    pub fn suggested_jitter(&self, frame_number: u32, render_resolution: [u32; 2]) -> [f32; 2] {
        let ratio = self.upscaled_resolution[0] as f32 / render_resolution[0] as f32;
        let phase_count = ((8.0 * ratio * ratio) as u32).max(32);
        let i = frame_number % phase_count;

        [halton_sequence(i, 2) - 0.5, halton_sequence(i, 3) - 0.5]
    }

    /// Suggested mip bias to apply when sampling textures.
    pub fn suggested_mip_bias(&self, render_resolution: [u32; 2]) -> f32 {
        (render_resolution[0] as f32 / self.upscaled_resolution[0] as f32).log2() - 1.0
    }

    /// The upscaled resolution DLSS will output at.
    pub fn upscaled_resolution(&self) -> [u32; 2] {
        self.upscaled_resolution
    }

    /// The resolution the camera should render at, pre-upscaling.
    pub fn render_resolution(&self) -> [u32; 2] {
        self.render_resolution
    }
}

impl Drop for DlssRayReconstruction {
    fn drop(&mut self) {
        unsafe {
            let hal_device = self.device.as_hal::<Vulkan>().unwrap();
            hal_device
                .raw_device()
                .device_wait_idle()
                .expect("Failed to wait for idle device when destroying DlssRayReconstruction");

            check_ngx_result(NVSDK_NGX_VULKAN_ReleaseFeature(self.feature))
                .expect("Failed to destroy DlssRayReconstruction feature");
        }
    }
}

unsafe impl Send for DlssRayReconstruction {}
unsafe impl Sync for DlssRayReconstruction {}

/// How roughness will be provided to [`DlssRayReconstruction`].
pub enum DlssRayReconstructionRoughnessMode {
    /// Roughness is provided as a standalone texture in [`DlssRayReconstructionRenderParameters::roughness`].
    Unpacked,
    /// Roughness is packed into the alpha channel of the normal texture in [`DlssRayReconstructionRenderParameters::normals`].
    Packed,
}

/// How depth will be provided to [`DlssRayReconstruction`].
pub enum DlssRayReconstructionDepthMode {
    /// Depth will be linear in view-space.
    Linear,
    /// Depth is a hardware depth buffer.
    Hardware,
}

/// Inputs and output resources needed for rendering [`DlssRayReconstruction`].
pub struct DlssRayReconstructionRenderParameters<'a> {
    /// Diffuse albedo.
    pub diffuse_albedo: &'a TextureView,
    /// Specular albedo.
    ///
    /// See section 3.4.2 of `$DLSS_SDK/doc/DLSS-RR Integration Guide.pdf` for how to calculate this texture.
    pub specular_albedo: &'a TextureView,
    /// Normals.
    ///
    /// Can be view-space or world-space.
    ///
    /// Must have linear material roughness in the alpha channel when using [`DlssRayReconstructionRoughnessMode::Packed`].
    pub normals: &'a TextureView,
    /// Linear material roughness.
    ///
    /// Must be provided when using [`DlssRayReconstructionRoughnessMode::Unpacked`].
    pub roughness: Option<&'a TextureView>,
    /// Main color view of your camera.
    pub color: &'a TextureView,
    /// Depth buffer.
    ///
    /// See [`DlssRayReconstructionDepthMode`] for format.
    pub depth: &'a TextureView,
    /// Motion vectors.
    pub motion_vectors: &'a TextureView,
    /// Specular material guide.
    pub specular_guide: DlssRayReconstructionSpecularGuide<'a>,
    /// Optional particles or other transparent effects, which are upscaled but not denoised.
    ///
    /// See section 3.4.10 of `$DLSS_SDK/doc/DLSS-RR Integration Guide.pdf`.
    pub transparency_overlay: Option<DlssRayReconstructionTransparencyOverlay<'a>>,
    /// Optional snapshot of [`Self::color`] before transparent effects are rendered on top of it.
    ///
    /// See section 3.4.11 of `$DLSS_SDK/doc/DLSS-RR Integration Guide.pdf`.
    pub color_before_transparency: Option<&'a TextureView>,
    /// Screen-space subsurface scattering guide.
    ///
    /// See section 3.4.12 of `$DLSS_SDK/doc/DLSS-RR Integration Guide.pdf` for how to calculate this texture
    pub screen_space_subsurface_scattering_guide: Option<&'a TextureView>,
    /// Optional depth of field guide.
    ///
    /// See section 3.4.13 of `$DLSS_SDK/doc/DLSS-RR Integration Guide.pdf` for how to calculate this texture.
    pub depth_of_field_guide: Option<&'a TextureView>,
    /// Optional per-pixel hint to make DLSS more or less responsive.
    ///
    /// See section 3.4.14 of `$DLSS_SDK/doc/DLSS-RR Integration Guide.pdf` for how to calculate this texture.
    pub responsivity_mask: Option<&'a TextureView>,
    /// Optional alpha texture to upscale, instead of the alpha channel of [`Self::color`].
    ///
    /// Requires [`DlssFeatureFlags::AlphaUpscaling`].
    pub alpha: Option<&'a TextureView>,
    /// Optional texture DLSS outputs alpha to, instead of the alpha channel of [`Self::dlss_output`].
    ///
    /// Requires [`DlssFeatureFlags::AlphaUpscaling`].
    pub dlss_output_alpha: Option<&'a TextureView>,
    /// The texture DLSS outputs to.
    pub dlss_output: &'a TextureView,
    /// Whether DLSS should reset temporal history, useful for camera cuts.
    pub reset: bool,
    /// Subpixel jitter that was applied to your camera.
    pub jitter_offset: [f32; 2],
    /// Optionally use only a specific subrect of the input textures, rather than the whole textures.
    // TODO: Allow configuring partial texture origins
    pub partial_texture_size: Option<[u32; 2]>,
    /// Optional scaling factor to apply to the values contained within [`Self::motion_vectors`].
    pub motion_vector_scale: Option<[f32; 2]>,
}

/// Guide buffer for specular material handling.
pub enum DlssRayReconstructionSpecularGuide<'a> {
    /// Motion vectors for objects reflected in specular material pixels.
    SpecularMotionVectors(&'a TextureView),
    /// World-space distance between primary vertex and hit point from tracing specular material pixels.
    SpecularHitDistance {
        /// Specular hit distance texture.
        texture_view: &'a TextureView,
        /// World-space to view-space camera matrix, as rows array.
        world_to_view_rows_array: [f32; 16],
        /// View-space to clip-space camera matrix, as rows array.
        view_to_clip_rows_array: [f32; 16],
    },
}

/// Transparent effects rendered separately from [`DlssRayReconstructionRenderParameters::color`].
pub enum DlssRayReconstructionTransparencyOverlay<'a> {
    /// RGB premultiplied by alpha, with alpha as the blending factor.
    Premultiplied(&'a TextureView),
    /// Color and per-channel opacity as two separate textures.
    Separate {
        /// Transparency color.
        color: &'a TextureView,
        /// Per-channel opacity.
        opacity: &'a TextureView,
    },
}

impl<'a> DlssRayReconstructionRenderParameters<'a> {
    fn validate(&self) -> Result<(), DlssError> {
        // TODO
        Ok(())
    }

    fn barrier_list(&self) -> impl Iterator<Item = TextureTransition<&'a Texture>> {
        fn resource_barrier(texture_view: &TextureView) -> TextureTransition<&Texture> {
            TextureTransition {
                texture: texture_view.texture(),
                selector: None,
                state: TextureUses::RESOURCE,
            }
        }

        fn storage_barrier(texture_view: &TextureView) -> TextureTransition<&Texture> {
            TextureTransition {
                texture: texture_view.texture(),
                selector: None,
                state: TextureUses::STORAGE_READ_WRITE,
            }
        }

        [
            Some(resource_barrier(self.diffuse_albedo)),
            Some(resource_barrier(self.specular_albedo)),
            Some(resource_barrier(self.normals)),
            self.roughness.map(resource_barrier),
            Some(resource_barrier(self.color)),
            Some(resource_barrier(self.depth)),
            Some(resource_barrier(self.motion_vectors)),
            match &self.specular_guide {
                DlssRayReconstructionSpecularGuide::SpecularMotionVectors(
                    specular_motion_vectors,
                ) => Some(resource_barrier(specular_motion_vectors)),
                DlssRayReconstructionSpecularGuide::SpecularHitDistance {
                    texture_view: specular_hit_distance,
                    ..
                } => Some(resource_barrier(specular_hit_distance)),
            },
            match &self.transparency_overlay {
                Some(DlssRayReconstructionTransparencyOverlay::Premultiplied(layer)) => {
                    Some(resource_barrier(layer))
                }
                Some(DlssRayReconstructionTransparencyOverlay::Separate { color, .. }) => {
                    Some(resource_barrier(color))
                }
                None => None,
            },
            match &self.transparency_overlay {
                Some(DlssRayReconstructionTransparencyOverlay::Separate { opacity, .. }) => {
                    Some(resource_barrier(opacity))
                }
                _ => None,
            },
            self.color_before_transparency.map(resource_barrier),
            self.screen_space_subsurface_scattering_guide
                .map(resource_barrier),
            self.depth_of_field_guide.map(resource_barrier),
            self.responsivity_mask.map(resource_barrier),
            self.alpha.map(resource_barrier),
            Some(storage_barrier(self.dlss_output)),
            self.dlss_output_alpha.map(storage_barrier),
        ]
        .into_iter()
        .flatten()
    }
}
