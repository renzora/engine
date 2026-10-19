//! Material graphs on 2D sprites, through Bevy 0.20's `SpriteMaterial`.
//!
//! A `.material` in the Sprite domain compiles to a sprite fragment shader
//! (`codegen::shaders::build_sprite_shader`). This module is the rest: the
//! [`SpriteGraphExt`] asset that carries its textures and parameters, and the
//! resolver that turns a [`MaterialRef`] on a `Sprite` into a
//! `SpriteMaterial<SpriteGraphExt>`.
//!
//! # Why a shader per material works here
//!
//! `MaterialExtension2d::fragment_shader` is a static fn: one shader for the
//! whole type. A graph material needs one per `.material`, so it is the 3D
//! trick again (`surface_ext.rs`): each compiled shader is inserted at its own
//! `Handle::Uuid`, the uuid rides in the bind-group key, and `specialize`
//! swaps the pipeline's fragment stage for it. Bevy's sprite wrapper forwards
//! the extension's key to `specialize` unchanged, which is what makes this
//! possible at all.
//!
//! # The sprite still decides
//!
//! The generated shader ends in `sprite_fn::get_final_color`, so the sprite's
//! own tint and alpha mode apply on top of the graph, and its flip, tiling and
//! slicing apply to the texture the graph reads. The graph's own alpha mode is
//! not used: on a sprite that setting belongs to the `Sprite` component.

use std::marker::PhantomData;

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2dKey, Material2dPipeline, MaterialExtension2d, SpriteMaterial, SpriteMaterialPlugin};
use uuid::Uuid;

use renzora::core::{MaterialRef, MaterialResolved};

use super::codegen::{self, TextureBinding, TextureKind};
use super::graph::{MaterialDomain, MaterialGraph};
use super::resolver::MaterialCache;
use super::runtime::FallbackTexture;
use super::surface_ext::SurfaceGraphParams;

/// The fragment shader a sprite material uses until its own compiles: the
/// sprite, drawn as Bevy would draw it.
pub const SPRITE_GRAPH_DEFAULT_FRAG: Handle<Shader> =
    bevy::asset::uuid_handle!("6f3d2a91-4c8e-4b7a-9d15-2e0c8b7a5f31");

const DEFAULT_SPRITE_FRAG_SRC: &str = r#"
import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;
import bevy_sprite_render::sprite_mesh::functions as sprite_fn;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return sprite_fn::sample_final_color(in.uv, in.instance_index);
}
"#;

/// A material graph's textures and parameters, for a sprite.
///
/// The same slots as the 3D [`super::surface_ext::SurfaceGraphExt`], counted
/// from 20 rather than 100: Bevy's sprite material already holds 0..2 (and 10
/// when bindless), and codegen declares these numbers from the same base
/// (`texture_bindings_at(ctx, .., 20)`). One shared sampler, for the same
/// Metal sampler limit the 3D struct documents.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(SpriteGraphExtKey)]
pub struct SpriteGraphExt {
    #[texture(20)]
    #[sampler(21)]
    pub texture_0: Option<Handle<Image>>,
    #[texture(22)]
    pub texture_1: Option<Handle<Image>>,
    #[texture(24)]
    pub texture_2: Option<Handle<Image>>,
    #[texture(26)]
    pub texture_3: Option<Handle<Image>>,
    #[texture(34)]
    pub texture_4: Option<Handle<Image>>,
    #[texture(36)]
    pub texture_5: Option<Handle<Image>>,
    #[texture(28, dimension = "cube")]
    pub cube_0: Option<Handle<Image>>,
    #[texture(30, dimension = "2d_array")]
    pub array_0: Option<Handle<Image>>,
    #[texture(32, dimension = "3d")]
    pub volume_0: Option<Handle<Image>>,
    #[uniform(38)]
    pub params: SurfaceGraphParams,
    /// The compiled shader's uuid; see the module docs.
    pub shader_uuid: Option<Uuid>,
}

/// What a sprite material's pipeline depends on. `Copy`, like the 3D key, so
/// it survives being packed into Bevy's combined bind-group data.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SpriteGraphExtKey {
    pub shader_uuid: Option<Uuid>,
}

impl From<&SpriteGraphExt> for SpriteGraphExtKey {
    fn from(ext: &SpriteGraphExt) -> Self {
        Self { shader_uuid: ext.shader_uuid }
    }
}

impl MaterialExtension2d for SpriteGraphExt {
    fn fragment_shader() -> Option<ShaderRef> {
        Some(SPRITE_GRAPH_DEFAULT_FRAG.into())
    }

    fn specialize(
        _pipeline: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let (Some(uuid), Some(fragment)) = (key.bind_group_data.shader_uuid, descriptor.fragment.as_mut()) {
            fragment.shader = Handle::<Shader>::Uuid(uuid, PhantomData);
        }
        Ok(())
    }
}

impl SpriteGraphExt {
    /// Every 2D slot on the fallback image, so the layout is complete before
    /// the graph's own textures are filled in.
    fn with_fallback(fallback: &Handle<Image>) -> Self {
        Self {
            texture_0: Some(fallback.clone()),
            texture_1: Some(fallback.clone()),
            texture_2: Some(fallback.clone()),
            texture_3: Some(fallback.clone()),
            texture_4: Some(fallback.clone()),
            texture_5: Some(fallback.clone()),
            ..default()
        }
    }
}

pub struct SpriteGraphMaterialPlugin;

impl Plugin for SpriteGraphMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpriteMaterialPlugin::<SpriteGraphExt>::default())
            .register_type::<SpriteMaterial<SpriteGraphExt>>()
            .add_systems(PostStartup, setup_default_sprite_shader)
            .add_systems(Update, (resolve_sprite_material_refs, clear_sprite_material_on_ref_removed));
    }
}

fn setup_default_sprite_shader(mut shaders: ResMut<Assets<Shader>>) {
    let shader = renzora::shader_source::shader(DEFAULT_SPRITE_FRAG_SRC, "graph_material/default_sprite.wesl");
    let _ = shaders.insert(&SPRITE_GRAPH_DEFAULT_FRAG, shader);
}

/// Give each sprite with a [`MaterialRef`] its `SpriteMaterial`.
///
/// The counterpart of the 3D `resolve_material_refs`, which skips sprites. A
/// failure still marks the entity resolved, as the 3D path does, so a broken
/// file is reported once rather than every frame; editing the file clears the
/// marker and it is tried again.
#[allow(clippy::too_many_arguments)]
fn resolve_sprite_material_refs(
    mut commands: Commands,
    query: Query<(Entity, &MaterialRef), (With<Sprite>, Without<MaterialResolved>)>,
    mut cache: ResMut<MaterialCache>,
    mut materials: ResMut<Assets<SpriteGraphExt>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut problems: ResMut<renzora::content_problems::ContentProblems>,
    fallback: Option<Res<FallbackTexture>>,
    project: Option<Res<renzora::CurrentProject>>,
    file_reader: Option<Res<renzora::VirtualFileReader>>,
    asset_server: Res<AssetServer>,
) {
    let default_reader = renzora::VirtualFileReader::default();
    let reader = file_reader.as_deref().unwrap_or(&default_reader);
    let fallback = fallback.map(|f| f.0.clone()).unwrap_or_default();
    for (entity, mat_ref) in &query {
        let path = &mat_ref.0;
        let handle = match cache.sprite_material(path) {
            Some(handle) => Some(handle),
            None => {
                let built = build_sprite_material(
                    path,
                    project.as_deref(),
                    reader,
                    &fallback,
                    &mut materials,
                    &mut shaders,
                    &mut problems,
                    &asset_server,
                );
                if let Some(handle) = &built {
                    cache.insert_sprite_material(path, handle.clone());
                }
                built
            }
        };
        let mut e = commands.entity(entity);
        match handle {
            Some(handle) => {
                e.try_insert(SpriteMaterial(handle));
            }
            // Pointed at something a sprite cannot wear: draw the plain sprite
            // rather than whatever material it had before.
            None => {
                e.try_remove::<SpriteMaterial<SpriteGraphExt>>();
            }
        }
        e.try_insert(MaterialResolved { source_path: path.clone() });
    }
}

/// Compile (or load the compiled form of) a Sprite-domain `.material` into a
/// [`SpriteGraphExt`]. `None`, with the reason in the Console, for anything
/// else.
#[allow(clippy::too_many_arguments)]
fn build_sprite_material(
    path: &str,
    project: Option<&renzora::CurrentProject>,
    reader: &renzora::VirtualFileReader,
    fallback: &Handle<Image>,
    materials: &mut Assets<SpriteGraphExt>,
    shaders: &mut Assets<Shader>,
    problems: &mut renzora::content_problems::ContentProblems,
    asset_server: &AssetServer,
) -> Option<Handle<SpriteGraphExt>> {
    let fail = |message: String| {
        renzora::console_log::console_error("Material", format!("{path}: {message}"));
        None
    };
    if !path.ends_with(".material") {
        return fail("a sprite can only wear a Sprite material (.material)".into());
    }
    let fs_path = match project {
        Some(project) if !std::path::Path::new(path).is_absolute() => {
            project.resolve_path(path).to_string_lossy().replace('\\', "/")
        }
        _ => path.to_string(),
    };
    let Some(content) = reader.read_string(&fs_path) else {
        return fail("could not be read".into());
    };
    let graph: MaterialGraph = match serde_json::from_str(&content) {
        Ok(graph) => graph,
        Err(e) => return fail(format!("is not a material graph ({e})")),
    };
    if graph.domain != MaterialDomain::Sprite {
        return fail(format!(
            "is a {} material; a sprite needs a Sprite material",
            graph.domain.display_name()
        ));
    }

    // The shader baked in on save, or a live compile for a graph that has
    // never been saved (or whose last compile failed), as the 3D path does.
    let (source, bindings, parameters) = match graph.compiled.as_ref() {
        Some(compiled) if compiled.meta.domain == MaterialDomain::Sprite => (
            compiled.wgsl.clone(),
            compiled.meta.texture_bindings.clone(),
            compiled.meta.parameters.clone(),
        ),
        _ => {
            let result = codegen::compile(&graph);
            if !result.errors.is_empty() {
                problems.set(
                    path.to_string(),
                    result
                        .errors
                        .iter()
                        .map(|message| renzora::content_problems::ContentProblem {
                            severity: renzora::content_problems::ProblemSeverity::Error,
                            message: message.clone(),
                            line: None,
                            node_id: None,
                        })
                        .collect(),
                );
                return fail(result.errors.join("; "));
            }
            for warning in &result.warnings {
                renzora::console_log::console_warn("Material", format!("{path}: {warning}"));
            }
            (result.fragment_shader, result.texture_bindings, result.parameters)
        }
    };
    problems.clear_path(path);

    let shader_uuid = insert_sprite_shader(
        shaders,
        source,
        format!("material/{}.wesl", path.replace(['\\', ':'], "/").trim_start_matches('/')),
    );
    Some(materials.add(sprite_material_from_compiled(
        fallback,
        shader_uuid,
        &bindings,
        &parameters,
        path,
        asset_server,
    )))
}

/// Insert a compiled sprite fragment shader at a fresh uuid, for a
/// [`SpriteGraphExt::shader_uuid`] to point at. A fresh one each time, because
/// the uuid is the pipeline key: reusing it would leave a pipeline already
/// built for the old source in place.
pub fn insert_sprite_shader(shaders: &mut Assets<Shader>, source: String, label: String) -> Uuid {
    let shader_uuid = Uuid::new_v4();
    let shader = renzora::shader_source::shader(source, label);
    let _ = shaders.insert(&Handle::<Shader>::Uuid(shader_uuid, PhantomData), shader);
    shader_uuid
}

/// The [`SpriteGraphExt`] for a compiled Sprite graph: its shader, its textures
/// and its parameters' authored defaults. Shared by the scene resolver and the
/// Material Editor's preview, so the two cannot draw a material differently.
pub fn sprite_material_from_compiled(
    fallback: &Handle<Image>,
    shader_uuid: Uuid,
    bindings: &[TextureBinding],
    parameters: &[codegen::MaterialParam],
    label: &str,
    asset_server: &AssetServer,
) -> SpriteGraphExt {
    let mut material = SpriteGraphExt::with_fallback(fallback);
    material.shader_uuid = Some(shader_uuid);
    fill_textures(&mut material, bindings, label, asset_server);
    material.params.slots = super::instance::build_default_param_slots(parameters);
    material
}

/// The same slot assignment the 3D assembler makes, onto the sprite's slots.
fn fill_textures(material: &mut SpriteGraphExt, bindings: &[TextureBinding], path: &str, asset_server: &AssetServer) {
    for binding in bindings {
        if binding.asset_path.is_empty() {
            continue;
        }
        let handle: Handle<Image> = asset_server.load(&binding.asset_path);
        match (binding.kind, binding.binding) {
            (TextureKind::D2, 0) => material.texture_0 = Some(handle),
            (TextureKind::D2, 1) => material.texture_1 = Some(handle),
            (TextureKind::D2, 2) => material.texture_2 = Some(handle),
            (TextureKind::D2, 3) => material.texture_3 = Some(handle),
            (TextureKind::D2, 4) => material.texture_4 = Some(handle),
            (TextureKind::D2, 5) => material.texture_5 = Some(handle),
            (TextureKind::Cube, 0) => material.cube_0 = Some(handle),
            (TextureKind::D2Array, 0) => material.array_0 = Some(handle),
            (TextureKind::D3, 0) => material.volume_0 = Some(handle),
            _ => warn!(
                "Sprite material '{}' wants more textures than a sprite material provides ({:?} slot {})",
                path, binding.kind, binding.binding
            ),
        }
    }
}

/// Take the material off a sprite whose [`MaterialRef`] was removed, so it
/// goes back to drawing as a plain sprite.
fn clear_sprite_material_on_ref_removed(
    mut commands: Commands,
    mut removed: RemovedComponents<MaterialRef>,
    orphaned: Query<(), (With<SpriteMaterial<SpriteGraphExt>>, Without<MaterialRef>)>,
) {
    for entity in removed.read() {
        if orphaned.get(entity).is_ok() {
            commands.entity(entity).remove::<(SpriteMaterial<SpriteGraphExt>, MaterialResolved)>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole route in a real App: a Sprite `.material` on disk, a sprite
    /// pointing at it, and the sprite ends up wearing a compiled
    /// `SpriteMaterial` whose shader is registered. A Surface material on a
    /// sprite is refused, and removing the reference takes the material off.
    #[test]
    fn a_sprite_wears_its_sprite_material() {
        let dir = std::env::temp_dir().join(format!("renzora_sprite_material_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, domain| {
            let path = dir.join(name);
            let graph = MaterialGraph::new(name, domain);
            std::fs::write(&path, serde_json::to_string(&graph).unwrap()).unwrap();
            path.to_string_lossy().replace('\\', "/")
        };
        let glow = write("glow.material", MaterialDomain::Sprite);
        let rock = write("rock.material", MaterialDomain::Surface);

        let mut app = renzora_test_harness::headless_app_with(|app| {
            app.add_plugins(super::super::MaterialPlugin);
        });
        let sprite = app.world_mut().spawn((Sprite::default(), MaterialRef(glow))).id();
        let wrong = app.world_mut().spawn((Sprite::default(), MaterialRef(rock))).id();
        for _ in 0..3 {
            app.update();
        }

        let world = app.world();
        let worn = world
            .get::<SpriteMaterial<SpriteGraphExt>>(sprite)
            .expect("the sprite wears its material");
        let ext = world.resource::<Assets<SpriteGraphExt>>().get(&worn.0).expect("material asset");
        let uuid = ext.shader_uuid.expect("a compiled shader");
        assert!(
            world.resource::<Assets<Shader>>().get(&Handle::<Shader>::Uuid(uuid, PhantomData)).is_some(),
            "the compiled shader is registered at its uuid"
        );
        assert!(world.get::<SpriteMaterial<SpriteGraphExt>>(wrong).is_none());
        assert!(world.get::<MaterialResolved>(wrong).is_some(), "a refusal is not retried every frame");

        app.world_mut().entity_mut(sprite).remove::<MaterialRef>();
        app.update();
        assert!(app.world().get::<SpriteMaterial<SpriteGraphExt>>(sprite).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
