use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::io::ErrorKind;
use std::path::Path;

use base64::Engine as _;
use bevy::reflect::TypePath;
use bevy::{
    asset::{AssetLoader, AssetPath, LoadContext, io::Reader},
    prelude::*,
};
use gltf::{
    Accessor,
    accessor::{DataType, Dimensions, Item, Iter},
    buffer::Source,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::gaussian::{
    formats::planar_3d::{Gaussian3d, PlanarGaussian3d, PlanarGaussian3dHandle},
    settings::{CloudSettings, GaussianColorSpace, GaussianMode},
};
use crate::material::spherical_harmonics::{
    SH_CHANNELS, SH_COEFF_COUNT, SH_COEFF_COUNT_PER_CHANNEL,
};

const KHR_GAUSSIAN_SPLATTING_EXTENSION: &str = "KHR_gaussian_splatting";

const ATTR_POSITION: &str = "POSITION";
const ATTR_COLOR_0: &str = "COLOR_0";
const ATTR_ROTATION: &str = "KHR_gaussian_splatting:ROTATION";
const ATTR_SCALE: &str = "KHR_gaussian_splatting:SCALE";
const ATTR_OPACITY: &str = "KHR_gaussian_splatting:OPACITY";
const ATTR_SH_PREFIX: &str = "KHR_gaussian_splatting:SH_DEGREE_";
const SH_DEGREE_ZERO_BASIS: f32 = 0.282_095;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Reflect)]
pub enum GaussianKernel {
    #[default]
    Ellipse,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Reflect)]
pub enum GaussianProjection {
    #[default]
    Perspective,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Reflect)]
pub enum GaussianSortingMethod {
    #[default]
    CameraDistance,
}

#[derive(Clone, Debug, Reflect)]
pub struct GaussianPrimitiveSpec {
    pub kernel: String,
    pub color_space: String,
    pub projection: String,
    pub sorting_method: String,
    #[reflect(ignore)]
    pub extension_object: Option<Value>,
}

impl Default for GaussianPrimitiveSpec {
    fn default() -> Self {
        Self {
            kernel: "ellipse".to_owned(),
            color_space: "srgb_rec709_display".to_owned(),
            projection: "perspective".to_owned(),
            sorting_method: "cameraDistance".to_owned(),
            extension_object: None,
        }
    }
}

#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct GaussianPrimitiveMetadata {
    pub kernel: GaussianKernel,
    pub projection: GaussianProjection,
    pub sorting_method: GaussianSortingMethod,
    pub spec: GaussianPrimitiveSpec,
}

#[derive(Clone, Debug, Default, Reflect)]
pub struct CloudBundle {
    pub cloud: Handle<PlanarGaussian3d>,
    pub name: String,
    pub settings: CloudSettings,
    pub transform: Transform,
    pub metadata: GaussianPrimitiveMetadata,
}

#[derive(Clone, Debug, Default, Reflect)]
pub struct SceneCamera {
    pub name: String,
    pub transform: Transform,
}

#[derive(Asset, Clone, Debug, Default, Reflect)]
pub struct GaussianScene {
    pub bundles: Vec<CloudBundle>,
    pub cameras: Vec<SceneCamera>,
}

#[derive(Clone, Debug)]
pub struct SceneExportCloud {
    pub cloud: PlanarGaussian3d,
    pub name: String,
    pub settings: CloudSettings,
    pub transform: Transform,
    pub metadata: GaussianPrimitiveMetadata,
}

#[derive(Clone, Debug)]
pub struct SceneExportCamera {
    pub name: String,
    pub transform: Transform,
    pub yfov_radians: f32,
    pub znear: f32,
    pub zfar: Option<f32>,
}

impl Default for SceneExportCamera {
    fn default() -> Self {
        Self {
            name: "camera".to_owned(),
            transform: Transform::default(),
            yfov_radians: std::f32::consts::FRAC_PI_4,
            znear: 0.01,
            zfar: Some(1000.0),
        }
    }
}

#[derive(Component, Clone, Debug, Default, Reflect)]
#[require(Transform, Visibility)]
pub struct GaussianSceneHandle(pub Handle<GaussianScene>);

#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct GaussianSceneLoaded;

#[derive(Default)]
pub struct GaussianScenePlugin;

impl Plugin for GaussianScenePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GaussianKernel>();
        app.register_type::<GaussianProjection>();
        app.register_type::<GaussianSortingMethod>();
        app.register_type::<GaussianPrimitiveSpec>();
        app.register_type::<GaussianPrimitiveMetadata>();
        app.register_type::<CloudBundle>();
        app.register_type::<SceneCamera>();
        app.register_type::<GaussianScene>();
        app.register_type::<GaussianSceneHandle>();
        app.register_type::<GaussianSceneLoaded>();

        app.init_asset::<GaussianScene>();
        app.init_asset_loader::<GaussianSceneLoader>();

        app.add_systems(Update, (spawn_scene,));
    }
}

fn spawn_scene(
    mut commands: Commands,
    scene_handles: Query<(Entity, &GaussianSceneHandle), Without<GaussianSceneLoaded>>,
    asset_server: Res<AssetServer>,
    scenes: Res<Assets<GaussianScene>>,
) {
    for (entity, scene_handle) in scene_handles.iter() {
        if let Some(load_state) = asset_server.get_load_state(&scene_handle.0)
            && !load_state.is_loaded()
        {
            continue;
        }

        let Some(scene) = scenes.get(&scene_handle.0) else {
            continue;
        };

        let bundles = scene.bundles.clone();

        commands
            .entity(entity)
            .with_children(move |builder| {
                for bundle in bundles {
                    builder.spawn((
                        PlanarGaussian3dHandle(bundle.cloud.clone()),
                        Name::new(bundle.name.clone()),
                        bundle.settings.clone(),
                        bundle.transform,
                        bundle.metadata.clone(),
                    ));
                }
            })
            .insert(GaussianSceneLoaded);
    }
}

#[derive(Default, TypePath)]
pub struct GaussianSceneLoader;

impl AssetLoader for GaussianSceneLoader {
    type Asset = GaussianScene;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        load_gltf_scene(&bytes, load_context).await
    }

    fn extensions(&self) -> &[&str] {
        &["gltf", "glb"]
    }
}

#[derive(Clone, Debug)]
struct GaussianPrimitiveSource {
    attributes: HashMap<String, usize>,
    metadata: GaussianPrimitiveMetadata,
    color_space: GaussianColorSpace,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRoot {
    #[serde(default, rename = "extensionsUsed")]
    extensions_used: Vec<String>,
    #[serde(default)]
    meshes: Vec<RawMesh>,
    #[serde(default)]
    nodes: Vec<RawNode>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMesh {
    #[serde(default)]
    primitives: Vec<RawPrimitive>,
}

#[derive(Debug, Default, Deserialize)]
struct RawNode {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPrimitive {
    #[serde(default)]
    attributes: HashMap<String, usize>,
    #[serde(default)]
    mode: Option<u32>,
    #[serde(default)]
    extensions: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGaussianExtension {
    kernel: String,
    color_space: String,
    #[serde(default = "default_projection")]
    projection: String,
    #[serde(default = "default_sorting_method")]
    sorting_method: String,
}

fn default_projection() -> String {
    "perspective".to_owned()
}

fn default_sorting_method() -> String {
    "cameraDistance".to_owned()
}

async fn load_gltf_scene(
    bytes: &[u8],
    load_context: &mut LoadContext<'_>,
) -> Result<GaussianScene, std::io::Error> {
    let raw_root = parse_raw_root(bytes)?;

    let gltf = gltf::Gltf::from_slice_without_validation(bytes).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to parse glTF document: {err}"),
        )
    })?;

    let primitive_sources = collect_gaussian_primitives(&raw_root)?;
    if primitive_sources.is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "no KHR_gaussian_splatting primitives found",
        ));
    }
    ensure_gaussian_extension_used(&raw_root.extensions_used)?;

    let buffers = load_buffers(&gltf, load_context).await?;
    let scene = gltf.default_scene().or_else(|| gltf.scenes().next());
    let Some(scene) = scene else {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "glTF does not contain any scenes",
        ));
    };

    let mut bundles = Vec::new();
    let mut cameras = Vec::new();
    let mut bundle_index = 0usize;

    for node in scene.nodes() {
        collect_node_bundles(
            &node,
            Mat4::IDENTITY,
            &raw_root,
            &gltf.document,
            &buffers,
            &primitive_sources,
            load_context,
            &mut bundle_index,
            &mut bundles,
            &mut cameras,
        )?;
    }

    if bundles.is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "KHR_gaussian_splatting scene contained no loadable gaussian primitives",
        ));
    }

    Ok(GaussianScene { bundles, cameras })
}

fn ensure_gaussian_extension_used(extensions_used: &[String]) -> Result<(), std::io::Error> {
    if extensions_used
        .iter()
        .any(|extension| extension == KHR_GAUSSIAN_SPLATTING_EXTENSION)
    {
        return Ok(());
    }

    Err(std::io::Error::new(
        ErrorKind::InvalidData,
        "KHR_gaussian_splatting primitives are present but the extension is missing from extensionsUsed",
    ))
}

fn parse_raw_root(bytes: &[u8]) -> Result<RawRoot, std::io::Error> {
    if bytes.starts_with(b"glTF") {
        let glb = gltf::binary::Glb::from_slice(bytes).map_err(|err| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("failed to parse GLB binary container: {err}"),
            )
        })?;

        serde_json::from_slice(glb.json.as_ref()).map_err(|err| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("failed to parse GLB JSON chunk: {err}"),
            )
        })
    } else {
        serde_json::from_slice(bytes).map_err(|err| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("failed to parse glTF JSON: {err}"),
            )
        })
    }
}

fn collect_gaussian_primitives(
    raw_root: &RawRoot,
) -> Result<HashMap<(usize, usize), GaussianPrimitiveSource>, std::io::Error> {
    let mut sources = HashMap::new();

    for (mesh_index, mesh) in raw_root.meshes.iter().enumerate() {
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            let Some(extension_value) = primitive.extensions.get(KHR_GAUSSIAN_SPLATTING_EXTENSION)
            else {
                continue;
            };

            let mode = primitive.mode.unwrap_or(4);
            if mode != 0 {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "mesh {mesh_index} primitive {primitive_index} has KHR_gaussian_splatting but mode={mode}; mode must be POINTS (0)"
                    ),
                ));
            }

            let extension: RawGaussianExtension =
                serde_json::from_value(extension_value.clone()).map_err(|err| {
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "mesh {mesh_index} primitive {primitive_index} has invalid KHR_gaussian_splatting extension payload: {err}"
                        ),
                    )
                })?;

            let kernel = parse_kernel(&extension.kernel, mesh_index, primitive_index)?;
            let color_space =
                parse_color_space(&extension.color_space, mesh_index, primitive_index)?;
            let projection = parse_projection(&extension.projection, mesh_index, primitive_index)?;
            let sorting_method =
                parse_sorting_method(&extension.sorting_method, mesh_index, primitive_index)?;

            sources.insert(
                (mesh_index, primitive_index),
                GaussianPrimitiveSource {
                    attributes: primitive.attributes.clone(),
                    metadata: GaussianPrimitiveMetadata {
                        kernel,
                        projection,
                        sorting_method,
                        spec: GaussianPrimitiveSpec {
                            kernel: extension.kernel.clone(),
                            color_space: extension.color_space.clone(),
                            projection: extension.projection.clone(),
                            sorting_method: extension.sorting_method.clone(),
                            extension_object: Some(extension_value.clone()),
                        },
                    },
                    color_space,
                },
            );
        }
    }

    Ok(sources)
}

fn parse_kernel(
    value: &str,
    mesh_index: usize,
    primitive_index: usize,
) -> Result<GaussianKernel, std::io::Error> {
    if value.trim().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "mesh {mesh_index} primitive {primitive_index} has an empty KHR_gaussian_splatting kernel value"
            ),
        ));
    }

    match value {
        "ellipse" => Ok(GaussianKernel::Ellipse),
        _ => {
            warn!(
                "mesh {} primitive {} uses extension kernel '{}'; falling back to base kernel 'ellipse'",
                mesh_index, primitive_index, value
            );
            Ok(GaussianKernel::Ellipse)
        }
    }
}

fn parse_color_space(
    value: &str,
    mesh_index: usize,
    primitive_index: usize,
) -> Result<GaussianColorSpace, std::io::Error> {
    if value.trim().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "mesh {mesh_index} primitive {primitive_index} has an empty KHR_gaussian_splatting colorSpace value"
            ),
        ));
    }

    match value {
        "srgb_rec709_display" => Ok(GaussianColorSpace::SrgbRec709Display),
        "lin_rec709_display" => Ok(GaussianColorSpace::LinRec709Display),
        _ => {
            warn!(
                "mesh {} primitive {} uses extension colorSpace '{}'; falling back to 'srgb_rec709_display'",
                mesh_index, primitive_index, value
            );
            Ok(GaussianColorSpace::SrgbRec709Display)
        }
    }
}

fn parse_projection(
    value: &str,
    mesh_index: usize,
    primitive_index: usize,
) -> Result<GaussianProjection, std::io::Error> {
    if value.trim().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "mesh {mesh_index} primitive {primitive_index} has an empty KHR_gaussian_splatting projection value"
            ),
        ));
    }

    match value {
        "perspective" => Ok(GaussianProjection::Perspective),
        _ => {
            warn!(
                "mesh {} primitive {} uses extension projection '{}'; falling back to 'perspective'",
                mesh_index, primitive_index, value
            );
            Ok(GaussianProjection::Perspective)
        }
    }
}

fn parse_sorting_method(
    value: &str,
    mesh_index: usize,
    primitive_index: usize,
) -> Result<GaussianSortingMethod, std::io::Error> {
    if value.trim().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "mesh {mesh_index} primitive {primitive_index} has an empty KHR_gaussian_splatting sortingMethod value"
            ),
        ));
    }

    match value {
        "cameraDistance" => Ok(GaussianSortingMethod::CameraDistance),
        _ => {
            warn!(
                "mesh {} primitive {} uses extension sortingMethod '{}'; falling back to 'cameraDistance'",
                mesh_index, primitive_index, value
            );
            Ok(GaussianSortingMethod::CameraDistance)
        }
    }
}

async fn load_buffers(
    gltf: &gltf::Gltf,
    load_context: &mut LoadContext<'_>,
) -> Result<Vec<Vec<u8>>, std::io::Error> {
    let mut buffers = Vec::new();
    let mut blob = gltf.blob.clone();

    for buffer in gltf.buffers() {
        let mut data = match buffer.source() {
            Source::Bin => blob.take().ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "glTF buffer references BIN chunk but binary data is missing",
                )
            })?,
            Source::Uri(uri) => {
                if let Some(decoded) = decode_data_uri(uri) {
                    decoded?
                } else {
                    let uri_path = AssetPath::try_parse(uri).map_err(|err| {
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("failed to resolve external buffer URI '{uri}': {err}"),
                        )
                    })?;
                    let path = load_context.path().resolve_embed(&uri_path);

                    load_context.read_asset_bytes(path).await.map_err(|err| {
                        std::io::Error::new(
                            ErrorKind::NotFound,
                            format!("failed to read external buffer '{uri}': {err}"),
                        )
                    })?
                }
            }
        };

        if data.len() < buffer.length() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "buffer {} length mismatch: expected at least {} bytes, got {} bytes",
                    buffer.index(),
                    buffer.length(),
                    data.len()
                ),
            ));
        }

        while data.len() % 4 != 0 {
            data.push(0);
        }

        buffers.push(data);
    }

    Ok(buffers)
}

fn decode_data_uri(uri: &str) -> Option<Result<Vec<u8>, std::io::Error>> {
    let rest = uri.strip_prefix("data:")?;

    let Some((metadata, payload)) = rest.split_once(',') else {
        return Some(Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "malformed data URI; expected a ',' separator",
        )));
    };

    let is_base64 = metadata
        .split(';')
        .any(|part| part.eq_ignore_ascii_case("base64"));

    if is_base64 {
        return Some(
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|err| {
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("failed to decode base64 data URI: {err}"),
                    )
                }),
        );
    }

    Some(decode_percent_encoded_data_uri(payload))
}

fn decode_percent_encoded_data_uri(payload: &str) -> Result<Vec<u8>, std::io::Error> {
    let bytes = payload.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "malformed percent-encoded data URI payload",
                ));
            }

            let high = decode_hex(bytes[index + 1])?;
            let low = decode_hex(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    Ok(decoded)
}

fn decode_hex(value: u8) -> Result<u8, std::io::Error> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "malformed percent-encoded data URI payload: invalid hex digit '{}'",
                value as char
            ),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_node_bundles(
    node: &gltf::Node<'_>,
    parent_transform: Mat4,
    raw_root: &RawRoot,
    document: &gltf::Document,
    buffers: &[Vec<u8>],
    primitive_sources: &HashMap<(usize, usize), GaussianPrimitiveSource>,
    load_context: &mut LoadContext<'_>,
    bundle_index: &mut usize,
    bundles: &mut Vec<CloudBundle>,
    cameras: &mut Vec<SceneCamera>,
) -> Result<(), std::io::Error> {
    let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
    let world_transform = parent_transform * local_transform;
    let node_name = raw_root
        .nodes
        .get(node.index())
        .and_then(|raw_node| raw_node.name.as_deref())
        .unwrap_or("gaussian_node");

    if node.camera().is_some() {
        cameras.push(SceneCamera {
            name: node_name.to_owned(),
            transform: Transform::from_matrix(world_transform),
        });
    }

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let key = (mesh.index(), primitive.index());
            let Some(source) = primitive_sources.get(&key) else {
                continue;
            };

            let cloud = decode_gaussian_primitive(document, buffers, source)?;
            let cloud_handle =
                load_context.add_labeled_asset(format!("gltf_gaussian_{}", *bundle_index), cloud);

            let settings = CloudSettings {
                gaussian_mode: GaussianMode::Gaussian3d,
                color_space: source.color_space,
                ..default()
            };

            bundles.push(CloudBundle {
                cloud: cloud_handle,
                name: format!(
                    "{node_name}_mesh{}_primitive{}",
                    mesh.index(),
                    primitive.index()
                ),
                settings,
                transform: Transform::from_matrix(world_transform),
                metadata: source.metadata.clone(),
            });
            *bundle_index += 1;
        }
    }

    for child in node.children() {
        collect_node_bundles(
            &child,
            world_transform,
            raw_root,
            document,
            buffers,
            primitive_sources,
            load_context,
            bundle_index,
            bundles,
            cameras,
        )?;
    }

    Ok(())
}

pub fn encode_khr_gaussian_scene_gltf_bytes(
    clouds: &[SceneExportCloud],
    camera: Option<&SceneExportCamera>,
) -> Result<Vec<u8>, std::io::Error> {
    if clouds.is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "cannot export an empty KHR_gaussian_splatting scene",
        ));
    }

    let mut binary = Vec::<u8>::new();
    let mut buffer_views = Vec::<Value>::new();
    let mut accessors = Vec::<Value>::new();
    let mut meshes = Vec::<Value>::new();
    let mut nodes = Vec::<Value>::new();
    let mut scene_nodes = Vec::<usize>::new();
    let mut cameras_json = Vec::<Value>::new();

    let export_sh_degree = max_export_sh_degree().min(3);
    let export_coeff_count = (export_sh_degree + 1) * (export_sh_degree + 1);

    for cloud in clouds {
        let source_gaussian_count = cloud.cloud.position_visibility.len();
        if source_gaussian_count == 0 {
            continue;
        }

        let mut positions = Vec::<f32>::with_capacity(source_gaussian_count * 3);
        let mut rotations = Vec::<f32>::with_capacity(source_gaussian_count * 4);
        let mut scales = Vec::<f32>::with_capacity(source_gaussian_count * 3);
        let mut opacities = Vec::<f32>::with_capacity(source_gaussian_count);
        let mut sh_channels = (0..export_coeff_count)
            .map(|_| Vec::<f32>::with_capacity(source_gaussian_count * 3))
            .collect::<Vec<_>>();

        let mut position_min = [f32::INFINITY; 3];
        let mut position_max = [f32::NEG_INFINITY; 3];
        let mut dropped_gaussians = 0usize;

        for gaussian in cloud.cloud.iter() {
            let rotation = gaussian.rotation.rotation;
            let rotation_length_sq = rotation
                .iter()
                .map(|component| component * component)
                .sum::<f32>();
            if rotation_length_sq <= f32::EPSILON || !rotation_length_sq.is_finite() {
                dropped_gaussians += 1;
                continue;
            }
            let inv_rotation_length = rotation_length_sq.sqrt().recip();
            let normalized_rotation = [
                rotation[0] * inv_rotation_length,
                rotation[1] * inv_rotation_length,
                rotation[2] * inv_rotation_length,
                rotation[3] * inv_rotation_length,
            ];

            let position = gaussian.position_visibility.position;
            positions.extend_from_slice(&position);
            for axis in 0..3 {
                position_min[axis] = position_min[axis].min(position[axis]);
                position_max[axis] = position_max[axis].max(position[axis]);
            }

            rotations.extend_from_slice(&normalized_rotation);

            scales.extend_from_slice(&[
                gaussian.scale_opacity.scale[0].max(1e-6).ln(),
                gaussian.scale_opacity.scale[1].max(1e-6).ln(),
                gaussian.scale_opacity.scale[2].max(1e-6).ln(),
            ]);
            opacities.push(gaussian.scale_opacity.opacity.clamp(0.0, 1.0));

            for (coefficient_index, channel) in sh_channels.iter_mut().enumerate() {
                let base = coefficient_index * SH_CHANNELS;
                channel.extend_from_slice(&[
                    gaussian.spherical_harmonic.coefficients[base],
                    gaussian.spherical_harmonic.coefficients[base + 1],
                    gaussian.spherical_harmonic.coefficients[base + 2],
                ]);
            }
        }

        let gaussian_count = positions.len() / 3;
        if gaussian_count == 0 {
            warn!(
                "skipping cloud '{}' during KHR export because all gaussians had invalid rotations",
                cloud.name
            );
            continue;
        }
        if dropped_gaussians > 0 {
            warn!(
                "dropped {} gaussians with invalid rotations while exporting cloud '{}'",
                dropped_gaussians, cloud.name
            );
        }

        let position_accessor = push_f32_accessor(
            &mut binary,
            &mut buffer_views,
            &mut accessors,
            AccessorSpec {
                values: &positions,
                count: gaussian_count,
                accessor_type: "VEC3",
                min: Some(position_min.to_vec()),
                max: Some(position_max.to_vec()),
            },
        );
        let rotation_accessor = push_f32_accessor(
            &mut binary,
            &mut buffer_views,
            &mut accessors,
            AccessorSpec {
                values: &rotations,
                count: gaussian_count,
                accessor_type: "VEC4",
                min: None,
                max: None,
            },
        );
        let scale_accessor = push_f32_accessor(
            &mut binary,
            &mut buffer_views,
            &mut accessors,
            AccessorSpec {
                values: &scales,
                count: gaussian_count,
                accessor_type: "VEC3",
                min: None,
                max: None,
            },
        );
        let opacity_accessor = push_f32_accessor(
            &mut binary,
            &mut buffer_views,
            &mut accessors,
            AccessorSpec {
                values: &opacities,
                count: gaussian_count,
                accessor_type: "SCALAR",
                min: None,
                max: None,
            },
        );

        let mut attributes = serde_json::Map::new();
        attributes.insert(ATTR_POSITION.to_owned(), json!(position_accessor));
        attributes.insert(ATTR_ROTATION.to_owned(), json!(rotation_accessor));
        attributes.insert(ATTR_SCALE.to_owned(), json!(scale_accessor));
        attributes.insert(ATTR_OPACITY.to_owned(), json!(opacity_accessor));

        for (coefficient_index, values) in sh_channels.iter().enumerate() {
            let sh_accessor = push_f32_accessor(
                &mut binary,
                &mut buffer_views,
                &mut accessors,
                AccessorSpec {
                    values,
                    count: gaussian_count,
                    accessor_type: "VEC3",
                    min: None,
                    max: None,
                },
            );
            let (degree, coefficient) = sh_index_to_degree_coefficient(coefficient_index);
            attributes.insert(
                format!("{ATTR_SH_PREFIX}{degree}_COEF_{coefficient}"),
                json!(sh_accessor),
            );
        }

        let primitive_extension =
            gaussian_extension_object(&cloud.metadata, cloud.settings.color_space);
        meshes.push(json!({
            "name": cloud.name,
            "primitives": [{
                "attributes": Value::Object(attributes),
                "mode": 0,
                "extensions": {
                    KHR_GAUSSIAN_SPLATTING_EXTENSION: primitive_extension
                }
            }]
        }));

        let node_index = nodes.len();
        scene_nodes.push(node_index);
        nodes.push(json!({
            "name": cloud.name,
            "mesh": meshes.len() - 1,
            "matrix": transform_matrix_values(cloud.transform),
        }));
    }

    if scene_nodes.is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "cannot export a KHR_gaussian_splatting scene with zero gaussians",
        ));
    }

    if let Some(camera) = camera {
        let mut perspective = serde_json::Map::new();
        perspective.insert("yfov".to_owned(), json!(camera.yfov_radians));
        perspective.insert("znear".to_owned(), json!(camera.znear));
        if let Some(zfar) = camera.zfar {
            perspective.insert("zfar".to_owned(), json!(zfar));
        }

        cameras_json.push(json!({
            "name": camera.name,
            "type": "perspective",
            "perspective": Value::Object(perspective),
        }));

        let camera_node_index = nodes.len();
        scene_nodes.push(camera_node_index);
        nodes.push(json!({
            "name": camera.name,
            "camera": cameras_json.len() - 1,
            "matrix": transform_matrix_values(camera.transform),
        }));
    }

    align_to_four_bytes(&mut binary);

    let mut root = serde_json::Map::new();
    root.insert("asset".to_owned(), json!({ "version": "2.0" }));
    root.insert(
        "extensionsUsed".to_owned(),
        json!([KHR_GAUSSIAN_SPLATTING_EXTENSION]),
    );
    root.insert(
        "extensionsRequired".to_owned(),
        json!([KHR_GAUSSIAN_SPLATTING_EXTENSION]),
    );
    root.insert("scene".to_owned(), json!(0));
    root.insert("scenes".to_owned(), json!([{ "nodes": scene_nodes }]));
    root.insert("nodes".to_owned(), Value::Array(nodes));
    root.insert("meshes".to_owned(), Value::Array(meshes));
    root.insert(
        "buffers".to_owned(),
        json!([{
            "byteLength": binary.len(),
            "uri": format!(
                "data:application/octet-stream;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&binary)
            ),
        }]),
    );
    root.insert("bufferViews".to_owned(), Value::Array(buffer_views));
    root.insert("accessors".to_owned(), Value::Array(accessors));
    if !cameras_json.is_empty() {
        root.insert("cameras".to_owned(), Value::Array(cameras_json));
    }

    serde_json::to_vec_pretty(&Value::Object(root)).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to serialize KHR_gaussian_splatting scene: {err}"),
        )
    })
}

pub fn write_khr_gaussian_scene_gltf(
    path: impl AsRef<Path>,
    clouds: &[SceneExportCloud],
    camera: Option<&SceneExportCamera>,
) -> Result<(), std::io::Error> {
    let bytes = encode_khr_gaussian_scene_gltf_bytes(clouds, camera)?;
    std::fs::write(path, bytes)
}

pub fn encode_khr_gaussian_scene_glb_bytes(
    clouds: &[SceneExportCloud],
    camera: Option<&SceneExportCamera>,
) -> Result<Vec<u8>, std::io::Error> {
    let gltf_bytes = encode_khr_gaussian_scene_gltf_bytes(clouds, camera)?;
    let mut root: Value = serde_json::from_slice(&gltf_bytes).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to parse generated KHR_gaussian_splatting glTF JSON: {err}"),
        )
    })?;

    let binary = extract_embedded_binary_buffer(&mut root)?;

    let json = serde_json::to_vec(&root).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to serialize KHR_gaussian_splatting GLB JSON chunk: {err}"),
        )
    })?;

    let glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(json),
        bin: Some(Cow::Owned(binary)),
    };

    glb.to_vec().map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to serialize KHR_gaussian_splatting GLB: {err}"),
        )
    })
}

pub fn write_khr_gaussian_scene_glb(
    path: impl AsRef<Path>,
    clouds: &[SceneExportCloud],
    camera: Option<&SceneExportCamera>,
) -> Result<(), std::io::Error> {
    let bytes = encode_khr_gaussian_scene_glb_bytes(clouds, camera)?;
    std::fs::write(path, bytes)
}

fn extract_embedded_binary_buffer(root: &mut Value) -> Result<Vec<u8>, std::io::Error> {
    let buffers = root
        .get_mut("buffers")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "missing glTF buffers array"))?;

    if buffers.len() != 1 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "KHR_gaussian_splatting export expects exactly one buffer, found {}",
                buffers.len()
            ),
        ));
    }

    let buffer = buffers[0].as_object_mut().ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidData, "buffer entry must be a JSON object")
    })?;
    let uri = buffer
        .remove("uri")
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "buffer URI missing; expected embedded data URI for GLB conversion",
            )
        })?;

    let binary = decode_data_uri(&uri).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            "buffer URI must be an embedded data URI for GLB conversion",
        )
    })??;

    buffer.insert("byteLength".to_owned(), json!(binary.len()));

    Ok(binary)
}

fn align_to_four_bytes(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

struct AccessorSpec<'a> {
    values: &'a [f32],
    count: usize,
    accessor_type: &'a str,
    min: Option<Vec<f32>>,
    max: Option<Vec<f32>>,
}

fn push_f32_accessor(
    binary: &mut Vec<u8>,
    buffer_views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    spec: AccessorSpec<'_>,
) -> usize {
    align_to_four_bytes(binary);
    let byte_offset = binary.len();
    for value in spec.values {
        binary.extend_from_slice(&value.to_le_bytes());
    }
    let byte_length = std::mem::size_of_val(spec.values);

    let buffer_view_index = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": byte_offset,
        "byteLength": byte_length,
    }));

    let mut accessor = serde_json::Map::new();
    accessor.insert("bufferView".to_owned(), json!(buffer_view_index));
    accessor.insert("componentType".to_owned(), json!(5126u32));
    accessor.insert("count".to_owned(), json!(spec.count));
    accessor.insert("type".to_owned(), json!(spec.accessor_type));
    if let Some(min) = spec.min {
        accessor.insert("min".to_owned(), json!(min));
    }
    if let Some(max) = spec.max {
        accessor.insert("max".to_owned(), json!(max));
    }

    let accessor_index = accessors.len();
    accessors.push(Value::Object(accessor));
    accessor_index
}

fn transform_matrix_values(transform: Transform) -> [f32; 16] {
    transform.to_matrix().to_cols_array()
}

fn gaussian_extension_object(
    metadata: &GaussianPrimitiveMetadata,
    color_space: GaussianColorSpace,
) -> Value {
    let mut extension_object = metadata
        .spec
        .extension_object
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    extension_object.insert(
        "kernel".to_owned(),
        Value::String(kernel_extension_identifier(metadata)),
    );
    extension_object.insert(
        "colorSpace".to_owned(),
        Value::String(color_space_extension_identifier(metadata, color_space)),
    );
    extension_object.insert(
        "projection".to_owned(),
        Value::String(projection_extension_identifier(metadata)),
    );
    extension_object.insert(
        "sortingMethod".to_owned(),
        Value::String(sorting_method_extension_identifier(metadata)),
    );

    Value::Object(extension_object)
}

fn kernel_extension_identifier(metadata: &GaussianPrimitiveMetadata) -> String {
    extension_identifier(
        &metadata.spec.kernel,
        kernel_to_extension_value(metadata.kernel),
        &["ellipse"],
    )
}

fn color_space_extension_identifier(
    metadata: &GaussianPrimitiveMetadata,
    color_space: GaussianColorSpace,
) -> String {
    extension_identifier(
        &metadata.spec.color_space,
        color_space_to_extension_value(color_space),
        &["srgb_rec709_display", "lin_rec709_display"],
    )
}

fn projection_extension_identifier(metadata: &GaussianPrimitiveMetadata) -> String {
    extension_identifier(
        &metadata.spec.projection,
        projection_to_extension_value(metadata.projection),
        &["perspective"],
    )
}

fn sorting_method_extension_identifier(metadata: &GaussianPrimitiveMetadata) -> String {
    extension_identifier(
        &metadata.spec.sorting_method,
        sorting_method_to_extension_value(metadata.sorting_method),
        &["cameraDistance"],
    )
}

fn extension_identifier(spec_value: &str, fallback_value: &str, known_values: &[&str]) -> String {
    let spec_value = spec_value.trim();
    if spec_value.is_empty() || known_values.contains(&spec_value) {
        fallback_value.to_owned()
    } else {
        spec_value.to_owned()
    }
}

fn kernel_to_extension_value(kernel: GaussianKernel) -> &'static str {
    match kernel {
        GaussianKernel::Ellipse => "ellipse",
    }
}

fn projection_to_extension_value(projection: GaussianProjection) -> &'static str {
    match projection {
        GaussianProjection::Perspective => "perspective",
    }
}

fn sorting_method_to_extension_value(method: GaussianSortingMethod) -> &'static str {
    match method {
        GaussianSortingMethod::CameraDistance => "cameraDistance",
    }
}

fn color_space_to_extension_value(color_space: GaussianColorSpace) -> &'static str {
    match color_space {
        GaussianColorSpace::SrgbRec709Display => "srgb_rec709_display",
        GaussianColorSpace::LinRec709Display => "lin_rec709_display",
    }
}

fn sh_index_to_degree_coefficient(index: usize) -> (usize, usize) {
    let mut degree = 0usize;
    while (degree + 1) * (degree + 1) <= index {
        degree += 1;
    }

    let coefficient = index - (degree * degree);
    (degree, coefficient)
}

fn max_export_sh_degree() -> usize {
    for degree in (0..=3).rev() {
        if (degree + 1) * (degree + 1) <= SH_COEFF_COUNT_PER_CHANNEL {
            return degree;
        }
    }
    0
}

fn decode_gaussian_primitive(
    document: &gltf::Document,
    buffers: &[Vec<u8>],
    source: &GaussianPrimitiveSource,
) -> Result<PlanarGaussian3d, std::io::Error> {
    let position_accessor = required_accessor(document, &source.attributes, ATTR_POSITION)?;
    let rotation_accessor = required_accessor(document, &source.attributes, ATTR_ROTATION)?;
    let scale_accessor = required_accessor(document, &source.attributes, ATTR_SCALE)?;
    let opacity_accessor = required_accessor(document, &source.attributes, ATTR_OPACITY)?;
    let sh_accessors = collect_sh_accessors(document, &source.attributes)?;

    let count = position_accessor.count();
    ensure_count(&rotation_accessor, count, ATTR_ROTATION)?;
    ensure_count(&scale_accessor, count, ATTR_SCALE)?;
    ensure_count(&opacity_accessor, count, ATTR_OPACITY)?;

    let positions = read_position_attribute(&position_accessor, buffers)?;
    let rotations = read_rotation_attribute(&rotation_accessor, buffers)?;
    let scales = read_scale_attribute(&scale_accessor, buffers)?;
    let opacities = read_opacity_attribute(&opacity_accessor, buffers)?;
    let color_fallback = if sh_accessors.is_empty() {
        match optional_accessor(document, &source.attributes, ATTR_COLOR_0)? {
            Some(color_accessor) => {
                ensure_count(&color_accessor, count, ATTR_COLOR_0)?;
                Some(read_color_attribute(&color_accessor, buffers)?)
            }
            None => None,
        }
    } else {
        None
    };

    let mut sh_channels = Vec::with_capacity(sh_accessors.len());
    for (coefficient_index, accessor) in sh_accessors {
        ensure_count(
            &accessor,
            count,
            &format!("{ATTR_SH_PREFIX}{coefficient_index}"),
        )?;
        sh_channels.push((
            coefficient_index,
            read_sh_coefficient_attribute(&accessor, buffers)?,
        ));
    }

    let mut gaussians = Vec::with_capacity(count);
    for index in 0..count {
        let mut spherical_harmonic =
            crate::material::spherical_harmonics::SphericalHarmonicCoefficients::default();

        if sh_channels.is_empty()
            && let Some(color_values) = &color_fallback
        {
            let color = color_values[index];
            spherical_harmonic.set(0, color[0] / SH_DEGREE_ZERO_BASIS);
            spherical_harmonic.set(1, color[1] / SH_DEGREE_ZERO_BASIS);
            spherical_harmonic.set(2, color[2] / SH_DEGREE_ZERO_BASIS);
        }

        for (coefficient_index, values) in &sh_channels {
            let rgb = values[index];
            let base = coefficient_index * SH_CHANNELS;
            if (base + 2) < SH_COEFF_COUNT {
                spherical_harmonic.set(base, rgb[0]);
                spherical_harmonic.set(base + 1, rgb[1]);
                spherical_harmonic.set(base + 2, rgb[2]);
            }
        }

        gaussians.push(Gaussian3d {
            position_visibility: [
                positions[index][0],
                positions[index][1],
                positions[index][2],
                1.0,
            ]
            .into(),
            spherical_harmonic,
            rotation: rotations[index].into(),
            scale_opacity: [
                scales[index][0],
                scales[index][1],
                scales[index][2],
                opacities[index],
            ]
            .into(),
        });
    }

    Ok(gaussians.into())
}

fn required_accessor<'a>(
    document: &'a gltf::Document,
    attributes: &HashMap<String, usize>,
    semantic: &str,
) -> Result<Accessor<'a>, std::io::Error> {
    let index = attributes.get(semantic).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("missing required attribute semantic '{semantic}'"),
        )
    })?;

    document.accessors().nth(*index).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("attribute semantic '{semantic}' references missing accessor index {index}"),
        )
    })
}

fn optional_accessor<'a>(
    document: &'a gltf::Document,
    attributes: &HashMap<String, usize>,
    semantic: &str,
) -> Result<Option<Accessor<'a>>, std::io::Error> {
    let Some(index) = attributes.get(semantic) else {
        return Ok(None);
    };

    let accessor = document.accessors().nth(*index).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("attribute semantic '{semantic}' references missing accessor index {index}"),
        )
    })?;

    Ok(Some(accessor))
}

fn collect_sh_accessors<'a>(
    document: &'a gltf::Document,
    attributes: &HashMap<String, usize>,
) -> Result<Vec<(usize, Accessor<'a>)>, std::io::Error> {
    let coefficient_map = collect_sh_coefficient_map(attributes)?;
    let mut accessors = Vec::with_capacity(coefficient_map.len());

    for (coefficient_index, accessor_index) in coefficient_map {
        let accessor = document.accessors().nth(accessor_index).ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("SH attribute references missing accessor index {accessor_index}"),
            )
        })?;

        accessors.push((coefficient_index, accessor));
    }

    Ok(accessors)
}

fn collect_sh_coefficient_map(
    attributes: &HashMap<String, usize>,
) -> Result<Vec<(usize, usize)>, std::io::Error> {
    let mut degrees: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();

    for (semantic, accessor_index) in attributes {
        let Some((degree, coefficient)) = parse_sh_semantic(semantic) else {
            continue;
        };

        degrees
            .entry(degree)
            .or_default()
            .insert(coefficient, *accessor_index);
    }

    if degrees.is_empty() {
        return Ok(Vec::new());
    }

    let Some(degree_zero) = degrees.get(&0) else {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "missing required spherical harmonics attribute 'KHR_gaussian_splatting:SH_DEGREE_0_COEF_0'",
        ));
    };
    if !degree_zero.contains_key(&0) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "missing required spherical harmonics attribute 'KHR_gaussian_splatting:SH_DEGREE_0_COEF_0'",
        ));
    }

    let max_degree = *degrees.keys().max().unwrap_or(&0);
    if max_degree > 3 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "unsupported spherical harmonics degree {max_degree}; KHR_gaussian_splatting supports degrees up to 3"
            ),
        ));
    }

    let supported_degree = max_supported_sh_degree();
    if max_degree > supported_degree {
        warn!(
            "asset uses spherical harmonics degree {max_degree}, but this build supports up to degree {supported_degree}; higher-degree coefficients will be discarded"
        );
    }

    for degree in 0..=max_degree {
        let expected_count = 2 * degree + 1;
        let Some(coefficients) = degrees.get(&degree) else {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "spherical harmonics degree {degree} is required because higher degrees are present, but its coefficients are missing"
                ),
            ));
        };

        if coefficients.len() != expected_count {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "spherical harmonics degree {degree} must define exactly {expected_count} coefficients"
                ),
            ));
        }

        for coefficient in 0..expected_count {
            if !coefficients.contains_key(&coefficient) {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "spherical harmonics degree {degree} is partially defined; missing coefficient {coefficient}"
                    ),
                ));
            }
        }
    }

    let mut coefficient_map = Vec::new();
    let map_max_degree = max_degree.min(supported_degree);
    for degree in 0..=map_max_degree {
        let coefficients = &degrees[&degree];
        for coefficient in 0..(2 * degree + 1) {
            let accessor_index = coefficients[&coefficient];
            coefficient_map.push((sh_coefficient_index(degree, coefficient), accessor_index));
        }
    }

    Ok(coefficient_map)
}

fn parse_sh_semantic(semantic: &str) -> Option<(usize, usize)> {
    let rest = semantic.strip_prefix(ATTR_SH_PREFIX)?;
    let (degree, coefficient) = rest.split_once("_COEF_")?;

    Some((degree.parse().ok()?, coefficient.parse().ok()?))
}

fn sh_coefficient_index(degree: usize, coefficient: usize) -> usize {
    degree * degree + coefficient
}

fn max_supported_sh_degree() -> usize {
    let mut degree = 0usize;
    loop {
        let coefficient_count = (degree + 1) * (degree + 1);
        if coefficient_count >= SH_COEFF_COUNT_PER_CHANNEL {
            return degree;
        }
        degree += 1;
    }
}

fn ensure_count(
    accessor: &Accessor<'_>,
    expected_count: usize,
    semantic: &str,
) -> Result<(), std::io::Error> {
    let count = accessor.count();
    if count == expected_count {
        return Ok(());
    }

    Err(std::io::Error::new(
        ErrorKind::InvalidData,
        format!("attribute semantic '{semantic}' has {count} entries; expected {expected_count}"),
    ))
}

fn read_position_attribute(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Result<Vec<[f32; 3]>, std::io::Error> {
    if accessor.dimensions() != Dimensions::Vec3 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_POSITION}' must use accessor type VEC3, got {:?}",
                accessor.dimensions()
            ),
        ));
    }

    if accessor.data_type() != DataType::F32 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_POSITION}' must use float components, got {:?}",
                accessor.data_type()
            ),
        ));
    }

    let values = read_items::<[f32; 3]>(accessor, buffers, ATTR_POSITION)?;
    if values
        .iter()
        .flatten()
        .any(|component| !component.is_finite())
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_POSITION}' contains non-finite values, which are invalid"
            ),
        ));
    }

    Ok(values)
}

fn read_rotation_attribute(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Result<Vec<[f32; 4]>, std::io::Error> {
    if accessor.dimensions() != Dimensions::Vec4 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_ROTATION}' must use accessor type VEC4, got {:?}",
                accessor.dimensions()
            ),
        ));
    }

    let normalized = accessor.normalized();
    let mut values = match accessor.data_type() {
        DataType::F32 => read_items::<[f32; 4]>(accessor, buffers, ATTR_ROTATION)?,
        DataType::I8 if normalized => read_items::<[i8; 4]>(accessor, buffers, ATTR_ROTATION)?
            .into_iter()
            .map(|v| {
                [
                    normalize_i8(v[0]),
                    normalize_i8(v[1]),
                    normalize_i8(v[2]),
                    normalize_i8(v[3]),
                ]
            })
            .collect(),
        DataType::I16 if normalized => read_items::<[i16; 4]>(accessor, buffers, ATTR_ROTATION)?
            .into_iter()
            .map(|v| {
                [
                    normalize_i16(v[0]),
                    normalize_i16(v[1]),
                    normalize_i16(v[2]),
                    normalize_i16(v[3]),
                ]
            })
            .collect(),
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "attribute semantic '{ATTR_ROTATION}' must use float or normalized signed integer components"
                ),
            ));
        }
    };

    let mut replaced_zero_length_quaternions = 0usize;
    for quaternion in &mut values {
        if normalize_quaternion(quaternion) {
            replaced_zero_length_quaternions += 1;
        }
    }
    if replaced_zero_length_quaternions > 0 {
        warn!(
            "attribute semantic '{}' contained {} zero-length quaternions; replacing them with identity rotations",
            ATTR_ROTATION, replaced_zero_length_quaternions
        );
    }

    if values
        .iter()
        .flatten()
        .any(|component| !component.is_finite())
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_ROTATION}' contains non-finite values, which are invalid"
            ),
        ));
    }

    Ok(values)
}

fn read_scale_attribute(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Result<Vec<[f32; 3]>, std::io::Error> {
    if accessor.dimensions() != Dimensions::Vec3 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_SCALE}' must use accessor type VEC3, got {:?}",
                accessor.dimensions()
            ),
        ));
    }

    let normalized = accessor.normalized();
    let mut values = match accessor.data_type() {
        DataType::F32 => read_items::<[f32; 3]>(accessor, buffers, ATTR_SCALE)?,
        DataType::I8 => read_items::<[i8; 3]>(accessor, buffers, ATTR_SCALE)?
            .into_iter()
            .map(|v| {
                if normalized {
                    [normalize_i8(v[0]), normalize_i8(v[1]), normalize_i8(v[2])]
                } else {
                    [v[0] as f32, v[1] as f32, v[2] as f32]
                }
            })
            .collect(),
        DataType::I16 => read_items::<[i16; 3]>(accessor, buffers, ATTR_SCALE)?
            .into_iter()
            .map(|v| {
                if normalized {
                    [
                        normalize_i16(v[0]),
                        normalize_i16(v[1]),
                        normalize_i16(v[2]),
                    ]
                } else {
                    [v[0] as f32, v[1] as f32, v[2] as f32]
                }
            })
            .collect(),
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "attribute semantic '{ATTR_SCALE}' must use float or signed integer components"
                ),
            ));
        }
    };

    for scale in &mut values {
        scale[0] = scale[0].exp();
        scale[1] = scale[1].exp();
        scale[2] = scale[2].exp();

        if !scale[0].is_finite() || !scale[1].is_finite() || !scale[2].is_finite() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "attribute semantic '{ATTR_SCALE}' produces non-finite scale after exp(scale), which is invalid"
                ),
            ));
        }
    }

    Ok(values)
}

fn read_opacity_attribute(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Result<Vec<f32>, std::io::Error> {
    if accessor.dimensions() != Dimensions::Scalar {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_OPACITY}' must use accessor type SCALAR, got {:?}",
                accessor.dimensions()
            ),
        ));
    }

    let normalized = accessor.normalized();
    let values: Vec<f32> = match accessor.data_type() {
        DataType::F32 => read_items::<f32>(accessor, buffers, ATTR_OPACITY)?,
        DataType::U8 if normalized => read_items::<u8>(accessor, buffers, ATTR_OPACITY)?
            .into_iter()
            .map(normalize_u8)
            .collect(),
        DataType::U16 if normalized => read_items::<u16>(accessor, buffers, ATTR_OPACITY)?
            .into_iter()
            .map(normalize_u16)
            .collect(),
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "attribute semantic '{ATTR_OPACITY}' must use float or normalized unsigned integer components"
                ),
            ));
        }
    };

    if values
        .iter()
        .any(|opacity| !opacity.is_finite() || *opacity < 0.0 || *opacity > 1.0)
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_OPACITY}' contains out-of-range values; opacity must be in [0, 1]"
            ),
        ));
    }

    Ok(values)
}

fn read_color_attribute(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Result<Vec<[f32; 3]>, std::io::Error> {
    let normalized = accessor.normalized();
    match accessor.dimensions() {
        Dimensions::Vec3 => {}
        Dimensions::Vec4 => {}
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "attribute semantic '{ATTR_COLOR_0}' must use accessor type VEC3 or VEC4, got {:?}",
                    accessor.dimensions()
                ),
            ));
        }
    }

    match (accessor.dimensions(), accessor.data_type()) {
        (Dimensions::Vec3, DataType::F32) => {
            read_items::<[f32; 3]>(accessor, buffers, ATTR_COLOR_0)
        }
        (Dimensions::Vec4, DataType::F32) => {
            Ok(read_items::<[f32; 4]>(accessor, buffers, ATTR_COLOR_0)?
                .into_iter()
                .map(|v| [v[0], v[1], v[2]])
                .collect())
        }
        (Dimensions::Vec3, DataType::U8) => {
            if !normalized {
                warn!(
                    "attribute semantic '{}' uses non-normalized U8 values; interpreting as normalized colors",
                    ATTR_COLOR_0
                );
            }
            Ok(read_items::<[u8; 3]>(accessor, buffers, ATTR_COLOR_0)?
                .into_iter()
                .map(|v| [normalize_u8(v[0]), normalize_u8(v[1]), normalize_u8(v[2])])
                .collect())
        }
        (Dimensions::Vec4, DataType::U8) => {
            if !normalized {
                warn!(
                    "attribute semantic '{}' uses non-normalized U8 values; interpreting as normalized colors",
                    ATTR_COLOR_0
                );
            }
            Ok(read_items::<[u8; 4]>(accessor, buffers, ATTR_COLOR_0)?
                .into_iter()
                .map(|v| [normalize_u8(v[0]), normalize_u8(v[1]), normalize_u8(v[2])])
                .collect())
        }
        (Dimensions::Vec3, DataType::U16) => {
            if !normalized {
                warn!(
                    "attribute semantic '{}' uses non-normalized U16 values; interpreting as normalized colors",
                    ATTR_COLOR_0
                );
            }
            Ok(read_items::<[u16; 3]>(accessor, buffers, ATTR_COLOR_0)?
                .into_iter()
                .map(|v| {
                    [
                        normalize_u16(v[0]),
                        normalize_u16(v[1]),
                        normalize_u16(v[2]),
                    ]
                })
                .collect())
        }
        (Dimensions::Vec4, DataType::U16) => {
            if !normalized {
                warn!(
                    "attribute semantic '{}' uses non-normalized U16 values; interpreting as normalized colors",
                    ATTR_COLOR_0
                );
            }
            Ok(read_items::<[u16; 4]>(accessor, buffers, ATTR_COLOR_0)?
                .into_iter()
                .map(|v| {
                    [
                        normalize_u16(v[0]),
                        normalize_u16(v[1]),
                        normalize_u16(v[2]),
                    ]
                })
                .collect())
        }
        _ => Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "attribute semantic '{ATTR_COLOR_0}' must use float, normalized unsigned byte, or normalized unsigned short components"
            ),
        )),
    }
}

fn read_sh_coefficient_attribute(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Result<Vec<[f32; 3]>, std::io::Error> {
    if accessor.dimensions() != Dimensions::Vec3 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "spherical harmonics attributes must use accessor type VEC3",
        ));
    }

    if accessor.data_type() != DataType::F32 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "spherical harmonics attributes must use float components",
        ));
    }

    let values = read_items::<[f32; 3]>(accessor, buffers, "KHR_gaussian_splatting:SH")?;
    if values
        .iter()
        .flatten()
        .any(|component| !component.is_finite())
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "spherical harmonics attributes contain non-finite values, which are invalid",
        ));
    }
    Ok(values)
}

fn read_items<T: Item + Copy>(
    accessor: &Accessor<'_>,
    buffers: &[Vec<u8>],
    semantic: &str,
) -> Result<Vec<T>, std::io::Error> {
    let iter = Iter::<T>::new(accessor.clone(), |buffer| {
        buffers.get(buffer.index()).map(Vec::as_slice)
    })
    .ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "failed to decode accessor data for attribute semantic '{semantic}' (accessor index {})",
                accessor.index()
            ),
        )
    })?;

    Ok(iter.collect())
}

fn normalize_quaternion(quaternion: &mut [f32; 4]) -> bool {
    let length_sq = quaternion
        .iter()
        .map(|component| component * component)
        .sum::<f32>();
    if length_sq <= f32::EPSILON {
        quaternion[0] = 1.0;
        quaternion[1] = 0.0;
        quaternion[2] = 0.0;
        quaternion[3] = 0.0;
        return true;
    }

    let inv_length = length_sq.sqrt().recip();
    quaternion[0] *= inv_length;
    quaternion[1] *= inv_length;
    quaternion[2] *= inv_length;
    quaternion[3] *= inv_length;

    false
}

fn normalize_i8(value: i8) -> f32 {
    (value as f32 / 127.0).max(-1.0)
}

fn normalize_i16(value: i16) -> f32 {
    (value as f32 / 32767.0).max(-1.0)
}

fn normalize_u8(value: u8) -> f32 {
    value as f32 / 255.0
}

fn normalize_u16(value: u16) -> f32 {
    value as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_complete_sh_coefficients_for_supported_degree() {
        let mut attributes = HashMap::new();
        attributes.insert(
            "KHR_gaussian_splatting:SH_DEGREE_0_COEF_0".to_owned(),
            0usize,
        );

        let supported_degree = max_supported_sh_degree().min(3);
        let mut index = 1usize;
        for degree in 1..=supported_degree {
            for coefficient in 0..(2 * degree + 1) {
                attributes.insert(
                    format!("KHR_gaussian_splatting:SH_DEGREE_{degree}_COEF_{coefficient}"),
                    index,
                );
                index += 1;
            }
        }

        let result = collect_sh_coefficient_map(&attributes).unwrap();
        assert_eq!(
            result.len(),
            (supported_degree + 1) * (supported_degree + 1)
        );
        assert_eq!(result[0].0, 0);
        assert_eq!(result.last().unwrap().0, result.len() - 1);
    }

    #[test]
    fn allows_no_sh_coefficients_for_extension_defined_lighting() {
        let attributes = HashMap::new();
        let result = collect_sh_coefficient_map(&attributes).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn rejects_partial_sh_degree() {
        let mut attributes = HashMap::new();
        attributes.insert(
            "KHR_gaussian_splatting:SH_DEGREE_0_COEF_0".to_owned(),
            0usize,
        );
        attributes.insert(
            "KHR_gaussian_splatting:SH_DEGREE_1_COEF_0".to_owned(),
            1usize,
        );
        attributes.insert(
            "KHR_gaussian_splatting:SH_DEGREE_1_COEF_2".to_owned(),
            2usize,
        );

        let err = collect_sh_coefficient_map(&attributes).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("must define exactly"));
    }

    #[test]
    fn clips_sh_coefficients_above_supported_degree() {
        let supported_degree = max_supported_sh_degree();
        if supported_degree >= 3 {
            return;
        }

        let mut attributes = HashMap::new();
        attributes.insert(
            "KHR_gaussian_splatting:SH_DEGREE_0_COEF_0".to_owned(),
            0usize,
        );

        let mut index = 1usize;
        for degree in 1..=3 {
            for coefficient in 0..(2 * degree + 1) {
                attributes.insert(
                    format!("KHR_gaussian_splatting:SH_DEGREE_{degree}_COEF_{coefficient}"),
                    index,
                );
                index += 1;
            }
        }

        let result = collect_sh_coefficient_map(&attributes).unwrap();
        assert_eq!(
            result.len(),
            (supported_degree + 1) * (supported_degree + 1)
        );
        assert!(
            result
                .iter()
                .all(|(index, _)| *index < (supported_degree + 1) * (supported_degree + 1))
        );
    }

    #[test]
    fn preserves_unknown_extension_identifiers_on_export() {
        let mut metadata = GaussianPrimitiveMetadata::default();
        metadata.spec.kernel = "customShape".to_owned();
        metadata.spec.color_space = "custom_space_display".to_owned();
        metadata.spec.projection = "customProjection".to_owned();
        metadata.spec.sorting_method = "customSort".to_owned();

        assert_eq!(kernel_extension_identifier(&metadata), "customShape");
        assert_eq!(
            color_space_extension_identifier(&metadata, GaussianColorSpace::SrgbRec709Display),
            "custom_space_display"
        );
        assert_eq!(
            projection_extension_identifier(&metadata),
            "customProjection"
        );
        assert_eq!(sorting_method_extension_identifier(&metadata), "customSort");
    }

    #[test]
    fn uses_runtime_color_space_for_known_identifiers() {
        let mut metadata = GaussianPrimitiveMetadata::default();
        metadata.spec.color_space = "lin_rec709_display".to_owned();

        assert_eq!(
            color_space_extension_identifier(&metadata, GaussianColorSpace::SrgbRec709Display),
            "srgb_rec709_display"
        );
        assert_eq!(
            color_space_extension_identifier(&metadata, GaussianColorSpace::LinRec709Display),
            "lin_rec709_display"
        );
    }

    #[test]
    fn decodes_base64_data_uri() {
        let uri = "data:application/octet-stream;base64,AAECAwQF";
        let data = decode_data_uri(uri).unwrap().unwrap();
        assert_eq!(data, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn decodes_percent_encoded_data_uri() {
        let uri = "data:application/octet-stream,%00%01%02%03";
        let data = decode_data_uri(uri).unwrap().unwrap();
        assert_eq!(data, vec![0, 1, 2, 3]);
    }

    #[test]
    fn requires_khr_extension_in_extensions_used() {
        let err = ensure_gaussian_extension_used(&[]).unwrap_err();
        assert!(err.to_string().contains("extensionsUsed"));

        ensure_gaussian_extension_used(&[KHR_GAUSSIAN_SPLATTING_EXTENSION.to_owned()]).unwrap();
    }

    #[test]
    fn encodes_khr_scene_with_camera_node() {
        let cloud: PlanarGaussian3d = vec![Gaussian3d {
            position_visibility: [1.0, 2.0, 3.0, 1.0].into(),
            spherical_harmonic:
                crate::material::spherical_harmonics::SphericalHarmonicCoefficients::default(),
            rotation: [1.0, 0.0, 0.0, 0.0].into(),
            scale_opacity: [1.0, 1.0, 1.0, 0.5].into(),
        }]
        .into();

        let export_cloud = SceneExportCloud {
            cloud,
            name: "cloud".to_owned(),
            settings: CloudSettings::default(),
            transform: Transform::default(),
            metadata: GaussianPrimitiveMetadata::default(),
        };
        let export_camera = SceneExportCamera {
            name: "camera".to_owned(),
            transform: Transform::from_xyz(4.0, 5.0, 6.0),
            ..default()
        };

        let bytes =
            encode_khr_gaussian_scene_gltf_bytes(&[export_cloud], Some(&export_camera)).unwrap();
        let root: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(
            root["extensionsUsed"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .next(),
            Some(KHR_GAUSSIAN_SPLATTING_EXTENSION)
        );
        assert_eq!(root["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(root["cameras"].as_array().unwrap().len(), 1);
        assert_eq!(root["nodes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn encodes_khr_scene_as_glb_with_bin_chunk() {
        let cloud: PlanarGaussian3d = vec![Gaussian3d {
            position_visibility: [1.0, 2.0, 3.0, 1.0].into(),
            spherical_harmonic:
                crate::material::spherical_harmonics::SphericalHarmonicCoefficients::default(),
            rotation: [1.0, 0.0, 0.0, 0.0].into(),
            scale_opacity: [1.0, 1.0, 1.0, 0.5].into(),
        }]
        .into();

        let export_cloud = SceneExportCloud {
            cloud,
            name: "cloud".to_owned(),
            settings: CloudSettings::default(),
            transform: Transform::default(),
            metadata: GaussianPrimitiveMetadata::default(),
        };
        let export_camera = SceneExportCamera {
            name: "camera".to_owned(),
            transform: Transform::from_xyz(4.0, 5.0, 6.0),
            ..default()
        };

        let glb_bytes =
            encode_khr_gaussian_scene_glb_bytes(&[export_cloud], Some(&export_camera)).unwrap();
        let glb = gltf::binary::Glb::from_slice(&glb_bytes).unwrap();

        assert!(glb.bin.is_some());

        let root: Value = serde_json::from_slice(glb.json.as_ref()).unwrap();
        assert_eq!(root["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(root["cameras"].as_array().unwrap().len(), 1);
        assert_eq!(root["nodes"].as_array().unwrap().len(), 2);

        let buffer = root["buffers"][0].as_object().unwrap();
        assert!(buffer.get("uri").is_none());
        assert!(buffer.get("byteLength").and_then(Value::as_u64).unwrap() > 0);
    }

    #[test]
    fn normalizes_zero_length_quaternion_to_identity() {
        let mut quaternion = [0.0, 0.0, 0.0, 0.0];
        let replaced = normalize_quaternion(&mut quaternion);
        assert!(replaced);
        assert_eq!(quaternion, [1.0, 0.0, 0.0, 0.0]);
    }

    fn build_gltf_with_accessors(
        buffer: Vec<u8>,
        buffer_views: Vec<(usize, usize)>,
        accessors: Vec<Value>,
    ) -> gltf::Gltf {
        let buffer_views_json: Vec<Value> = buffer_views
            .into_iter()
            .map(|(byte_offset, byte_length)| {
                json!({
                    "buffer": 0,
                    "byteOffset": byte_offset,
                    "byteLength": byte_length,
                })
            })
            .collect();

        let root = json!({
            "asset": { "version": "2.0" },
            "buffers": [
                { "byteLength": buffer.len() }
            ],
            "bufferViews": buffer_views_json,
            "accessors": accessors,
        });

        let bytes = serde_json::to_vec(&root).expect("failed to serialize glTF");
        let gltf = gltf::Gltf::from_slice_without_validation(&bytes).expect("failed to parse glTF");

        assert!(gltf.blob.is_none());
        gltf
    }

    #[test]
    fn reads_quantized_rotation_scale_opacity() {
        let rotation_bytes = [127i8, 0, 0, 0].map(|v| v as u8);
        let scale_bytes = [-127i8, 0, 127].map(|v| v as u8);
        let opacity_bytes = [128u8];

        let mut buffer = Vec::new();
        buffer.extend_from_slice(&rotation_bytes);
        let rotation_offset = 0usize;
        let scale_offset = buffer.len();
        buffer.extend_from_slice(&scale_bytes);
        buffer.push(0);
        buffer.push(0);
        buffer.push(0);
        let opacity_offset = buffer.len();
        buffer.extend_from_slice(&opacity_bytes);
        buffer.resize(buffer.len() + 3, 0);

        let gltf = build_gltf_with_accessors(
            buffer.clone(),
            vec![
                (rotation_offset, rotation_bytes.len()),
                (scale_offset, scale_bytes.len()),
                (opacity_offset, opacity_bytes.len()),
            ],
            vec![
                json!({
                    "bufferView": 0,
                    "componentType": 5120,
                    "count": 1,
                    "type": "VEC4",
                    "normalized": true,
                }),
                json!({
                    "bufferView": 1,
                    "componentType": 5120,
                    "count": 1,
                    "type": "VEC3",
                    "normalized": true,
                }),
                json!({
                    "bufferView": 2,
                    "componentType": 5121,
                    "count": 1,
                    "type": "SCALAR",
                    "normalized": true,
                }),
            ],
        );

        let accessors: Vec<_> = gltf.document.accessors().collect();
        let buffers = vec![buffer];

        let rotations = read_rotation_attribute(&accessors[0], &buffers).unwrap();
        assert!((rotations[0][0] - 1.0).abs() < 1e-6);
        assert!(rotations[0][1].abs() < 1e-6);
        assert!(rotations[0][2].abs() < 1e-6);
        assert!(rotations[0][3].abs() < 1e-6);

        let scales = read_scale_attribute(&accessors[1], &buffers).unwrap();
        assert!((scales[0][0] - (-1.0f32).exp()).abs() < 1e-6);
        assert!((scales[0][1] - 0.0f32.exp()).abs() < 1e-6);
        assert!((scales[0][2] - 1.0f32.exp()).abs() < 1e-6);

        let opacities = read_opacity_attribute(&accessors[2], &buffers).unwrap();
        assert!((opacities[0] - (128.0 / 255.0)).abs() < 1e-6);
    }

    #[test]
    fn reads_unnormalized_scale_i16() {
        let scale_values: [i16; 3] = [-2, 0, 2];
        let mut buffer = Vec::new();
        for value in scale_values {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        buffer.resize(buffer.len() + 2, 0);

        let gltf = build_gltf_with_accessors(
            buffer.clone(),
            vec![(0, scale_values.len() * std::mem::size_of::<i16>())],
            vec![json!({
                "bufferView": 0,
                "componentType": 5122,
                "count": 1,
                "type": "VEC3",
            })],
        );

        let accessors: Vec<_> = gltf.document.accessors().collect();
        let scales = read_scale_attribute(&accessors[0], &[buffer]).unwrap();
        assert!((scales[0][0] - (-2.0f32).exp()).abs() < 1e-6);
        assert!((scales[0][1] - 0.0f32.exp()).abs() < 1e-6);
        assert!((scales[0][2] - 2.0f32.exp()).abs() < 1e-6);
    }

    #[test]
    fn skips_invalid_rotation_gaussians_during_export() {
        let invalid = Gaussian3d {
            position_visibility: [0.0, 0.0, 0.0, 1.0].into(),
            spherical_harmonic:
                crate::material::spherical_harmonics::SphericalHarmonicCoefficients::default(),
            rotation: [0.0, 0.0, 0.0, 0.0].into(),
            scale_opacity: [1.0, 1.0, 1.0, 1.0].into(),
        };
        let valid = Gaussian3d {
            position_visibility: [1.0, 2.0, 3.0, 1.0].into(),
            spherical_harmonic:
                crate::material::spherical_harmonics::SphericalHarmonicCoefficients::default(),
            rotation: [1.0, 0.0, 0.0, 0.0].into(),
            scale_opacity: [1.0, 1.0, 1.0, 1.0].into(),
        };
        let cloud: PlanarGaussian3d = vec![invalid, valid].into();

        let export_cloud = SceneExportCloud {
            cloud,
            name: "cloud".to_owned(),
            settings: CloudSettings::default(),
            transform: Transform::default(),
            metadata: GaussianPrimitiveMetadata::default(),
        };

        let bytes = encode_khr_gaussian_scene_gltf_bytes(&[export_cloud], None).unwrap();
        let root: Value = serde_json::from_slice(&bytes).unwrap();
        let rotation_accessor_index = root["meshes"][0]["primitives"][0]["attributes"]
            [ATTR_ROTATION]
            .as_u64()
            .unwrap() as usize;
        let position_accessor_index = root["meshes"][0]["primitives"][0]["attributes"]
            [ATTR_POSITION]
            .as_u64()
            .unwrap() as usize;

        assert_eq!(
            root["accessors"][rotation_accessor_index]["count"]
                .as_u64()
                .unwrap(),
            1
        );
        assert_eq!(
            root["accessors"][position_accessor_index]["count"]
                .as_u64()
                .unwrap(),
            1
        );
    }
}
