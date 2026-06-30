//! Bevy-native (ember) port of the egui `ShaderPreviewPanel`: a mesh-selector
//! toolbar over the shader preview render texture (`ShaderPreviewImage.handle`),
//! with no-shader / incompatible empty states and a language + filename footer.

use bevy::prelude::*;

use renzora::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_with};
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu};

use crate::preview::{PreviewMesh, ShaderPreviewImage};
use crate::ShaderEditorState;

pub struct NativeShaderPreview;

impl Plugin for NativeShaderPreview {
    fn build(&self, app: &mut App) {
        app.register_panel_content("shader_preview", false, build);
        app.add_systems(Update, mesh_combo_open.run_if(in_state(SplashState::Editor)));
    }
}

#[derive(Component)]
struct MeshComboBtn;

fn has_shader(w: &World) -> bool {
    w.get_resource::<ShaderEditorState>().is_some_and(|s| s.compiled_wgsl.is_some())
}
fn compatible(w: &World) -> bool {
    w.get_resource::<ShaderEditorState>().is_none_or(|s| s.preview_compatible)
}

fn incompat_msg(w: &World) -> String {
    use renzora_shader::file::ShaderType;
    match w.get_resource::<ShaderEditorState>().map(|s| s.shader_file.shader_type) {
        Some(ShaderType::Material) => renzora::lang::t("shader_preview.incompat_material"),
        Some(ShaderType::PostProcess) => renzora::lang::t("shader_preview.incompat_postprocess"),
        _ => renzora::lang::t("shader_preview.incompat_generic"),
    }
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }, Name::new("native-shader-preview")))
        .id();

    // No-shader state.
    let no_shader = note(commands, fonts, &renzora::lang::t("shader_preview.no_compiled"));
    bind_display(commands, no_shader, |w| !has_shader(w));

    // Incompatible state (has a shader but it can't preview).
    let incompat = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::all(Val::Px(12.0)), ..default() })
        .id();
    let incompat_lbl = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::justify(bevy::text::Justify::Center)))
        .id();
    bind_text(commands, incompat_lbl, incompat_msg);
    commands.entity(incompat).add_child(incompat_lbl);
    bind_display(commands, incompat, |w| has_shader(w) && !compatible(w));

    // Body: mesh toolbar + image + footer.
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    bind_display(commands, body, |w| has_shader(w) && compatible(w));

    // Toolbar: Mesh combo.
    let toolbar = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), flex_shrink: 0.0, ..default() })
        .id();
    let mesh_lbl = commands.spawn((Text::new(renzora::lang::t("shader_preview.mesh")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    let combo = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            MeshComboBtn,
        ))
        .id();
    let combo_v = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { min_width: Val::Px(74.0), ..default() })).id();
    bind_text(commands, combo_v, |w| {
        w.get_resource::<ShaderEditorState>().map(|s| s.preview_mesh.label()).unwrap_or("Sphere").to_string()
    });
    let combo_c = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[combo_v, combo_c]);
    commands.entity(toolbar).add_children(&[mesh_lbl, combo]);

    // Preview image — square, centered.
    let img_box = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() })
        .id();
    let img = commands
        .spawn((
            ImageNode::default(),
            Node { width: Val::Percent(100.0), aspect_ratio: Some(1.0), ..default() },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            Name::new("shader-preview-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<ShaderPreviewImage>().map(|p| p.handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );
    commands.entity(img_box).add_child(img);

    // Footer: language + filename.
    let footer = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), flex_shrink: 0.0, ..default() }))
        .id();
    bind_text(commands, footer, |w| {
        let Some(s) = w.get_resource::<ShaderEditorState>() else { return String::new() };
        let name = s.file_path.as_ref().and_then(|p| std::path::Path::new(p).file_name().and_then(|n| n.to_str())).unwrap_or("");
        if name.is_empty() { s.shader_file.language.clone() } else { format!("{}  \u{b7}  {}", s.shader_file.language, name) }
    });

    commands.entity(body).add_children(&[toolbar, img_box, footer]);
    commands.entity(root).add_children(&[no_shader, incompat, body]);
    root
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let n = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() })
        .id();
    let l = commands.spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::justify(bevy::text::Justify::Center))).id();
    commands.entity(n).add_child(l);
    n
}

fn mesh_combo_open(
    q: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &bevy::ui::ComputedNode), (With<MeshComboBtn>, Changed<Interaction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = PreviewMesh::ALL
        .iter()
        .map(|&mesh| {
            menu_item(&mut commands, &fonts, "cube", mesh.label(), move |w| {
                if let Some(mut s) = w.get_resource_mut::<ShaderEditorState>() {
                    s.preview_mesh = mesh;
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}
