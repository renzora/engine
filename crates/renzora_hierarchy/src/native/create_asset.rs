//! "Attach ▸ …" on the hierarchy's right-click menu: make a new project asset
//! (script, blueprint, material, particle, UI template, scene) and — for the
//! kinds an entity can actually carry — wire it onto the entity you clicked, in
//! the same step.
//!
//! Two stages, because the two questions are different. The context submenu
//! picks the *kind*; an overlay then asks *what to call it* and *where it goes*.
//! The destination is ember's shared [`folder_picker`] — the same tree the
//! marketplace installs into — so "somewhere sensible, but not necessarily the
//! folder the asset browser happens to be showing" is one click, and the file
//! doesn't silently land in whatever directory another panel was last parked in.
//!
//! Creating from the hierarchy rather than the assets panel is the whole point:
//! a fresh script exists *and* is attached to the selected entity without a
//! round-trip through the browser and a drag onto the inspector.

use std::path::{Path, PathBuf};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::Bound;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    button, checkbox, folder_new_button, folder_picker, menu_item_styled, menu_submenu_styled, overlay_sized,
    text_input, EmberForm, EmberTextInput, FolderPick, Overlay,
};
use renzora_scripting::ScriptComponent;

/// How deep the destination tree walks below the project root. Deep enough to
/// reach `assets/particles/2d` without expanding anything, shallow enough that
/// the list stays scannable.
const PICKER_DEPTH: usize = 2;

/// What the Create submenu can make. Mirrors the assets panel's own Add menu
/// (labels, icons and accents come from the same `assets.new.*` strings) so the
/// two entry points read as one feature rather than two similar ones.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreateKind {
    Script,
    /// A `.rs` script — `renzora_rust_script` compiles it to a native plugin and
    /// calls it with `&mut World` once per frame per entity. Routing is by file
    /// extension, so it attaches through the same `ScriptComponent` as Lua and
    /// the two coexist on one entity.
    RustScript,
    Blueprint,
    Material,
    Particle,
    Template,
    Scene,
}

impl CreateKind {
    const ALL: [CreateKind; 7] = [
        CreateKind::Script,
        CreateKind::RustScript,
        CreateKind::Blueprint,
        CreateKind::Material,
        CreateKind::Particle,
        CreateKind::Template,
        CreateKind::Scene,
    ];

    fn label(self) -> String {
        match self {
            CreateKind::Script => renzora::lang::t("assets.new.lua"),
            CreateKind::RustScript => renzora::lang::t_or("assets.new.rust", "Rust Script"),
            CreateKind::Blueprint => renzora::lang::t("assets.new.blueprint"),
            CreateKind::Material => renzora::lang::t("assets.new.material"),
            CreateKind::Particle => renzora::lang::t("assets.new.particle"),
            CreateKind::Template => renzora::lang::t("assets.new.template"),
            CreateKind::Scene => renzora::lang::t("assets.new.bsn"),
        }
    }

    /// Worth offering on a UI entity.
    ///
    /// A canvas or a widget can carry scripts and can be given a template; a
    /// material, a particle system, a blueprint or a nested scene are things you
    /// attach to a *scene* entity, and offering them on a canvas is offering
    /// something that will not do anything.
    fn on_ui(self) -> bool {
        matches!(
            self,
            CreateKind::Script | CreateKind::RustScript | CreateKind::Template
        )
    }

    /// Default file name (without extension) the name field starts with.
    fn stem(self) -> &'static str {
        match self {
            CreateKind::Script => "new_script",
            CreateKind::RustScript => "new_script",
            CreateKind::Blueprint => "NewBlueprint",
            CreateKind::Material => "NewMaterial",
            CreateKind::Particle => "NewParticle",
            CreateKind::Template => "NewTemplate",
            CreateKind::Scene => "NewScene",
        }
    }

    fn ext(self) -> &'static str {
        match self {
            CreateKind::Script => "lua",
            CreateKind::RustScript => "rs",
            CreateKind::Blueprint => "blueprint",
            CreateKind::Material => "material",
            CreateKind::Particle => "particle",
            CreateKind::Template => "html",
            CreateKind::Scene => "bsn",
        }
    }

    /// Starter contents — byte-identical to what the assets panel writes, so a
    /// file created here opens in exactly the same state as one created there.
    fn content(self) -> String {
        match self {
            CreateKind::Script => "-- New Lua script\n".to_string(),
            // A complete, compiling script — not a stub. `renzora::script!` is
            // what exports the entry point, and a `.rs` without it loads and
            // then reports "exports no entry point", which is a poor first
            // impression of a feature whose whole promise is that it compiles.
            CreateKind::RustScript => concat!(
                "use bevy::prelude::*;\n",
                "use renzora::ScriptCtx;\n",
                "\n",
                "fn update(ctx: &mut ScriptCtx) {\n",
                "    let _dt = ctx.delta();\n",
                "}\n",
                "\n",
                "renzora::script!(update);\n",
            )
            .to_string(),
            // Not `{}`: a blueprint with no event node is a dead canvas, so new
            // files start with On Ready + On Update already placed. Owned by
            // the blueprint crate so both create paths write the same graph.
            CreateKind::Blueprint => renzora_blueprint::starter_blueprint_json(),
            CreateKind::Material => "{}".to_string(),
            CreateKind::Particle => "(name: \"New Particle\")".to_string(),
            CreateKind::Template => "<template>\n    <node></node>\n</template>\n".to_string(),
            // An empty scene = just the interim-BSN header the parser expects.
            CreateKind::Scene => "// renzora interim bsn v1\n".to_string(),
        }
    }

    /// Conventional project subfolder, pre-selected in the picker (and created
    /// if missing, so it's visible in the tree on a fresh project). Same slugs
    /// the marketplace installs into.
    fn default_dir(self) -> &'static str {
        match self {
            CreateKind::Script | CreateKind::RustScript => "scripts",
            CreateKind::Blueprint => "blueprints",
            CreateKind::Material => "materials",
            CreateKind::Particle => "particles",
            CreateKind::Template => "ui",
            CreateKind::Scene => "scenes",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            CreateKind::Script | CreateKind::Template => "code",
            CreateKind::RustScript => "gear-six",
            CreateKind::Blueprint => "blueprint",
            CreateKind::Material => "palette",
            CreateKind::Particle => "sparkle",
            CreateKind::Scene => "film-slate",
        }
    }

    fn color(self) -> (u8, u8, u8) {
        match self {
            CreateKind::Script => (120, 170, 255),
            // Rust's orange, so the two script kinds are one glance apart.
            CreateKind::RustScript => (222, 130, 80),
            CreateKind::Blueprint => (100, 180, 255),
            CreateKind::Material => (0, 200, 130),
            CreateKind::Particle => (230, 160, 90),
            CreateKind::Template => (230, 120, 90),
            CreateKind::Scene => (115, 191, 242),
        }
    }

    /// Whether the entity can carry this asset. Scripts and blueprints both run
    /// through `ScriptComponent`; the rest are project files with no direct
    /// entity attachment, so their overlay omits the attach row entirely.
    fn attachable(self) -> bool {
        matches!(
            self,
            CreateKind::Script | CreateKind::RustScript | CreateKind::Blueprint
        )
    }
}

/// The overlay awaiting confirmation. Lives only while it's on screen — Escape,
/// a backdrop click or the X despawn the overlay, and [`create_overlay_reap`]
/// drops this behind it.
#[derive(Resource)]
pub(crate) struct PendingCreate {
    kind: CreateKind,
    /// The right-clicked entity, for the attach step.
    target: Entity,
    overlay: Entity,
    name_input: Entity,
    /// The "attach to entity" checkbox, when this kind has one.
    attach: Option<Entity>,
    /// Update ticks since the overlay opened, counted only so the auto-focus
    /// lands on the *second* one — see [`focus_name_field`].
    ticks: u32,
}

#[derive(Component)]
pub(crate) struct CreateConfirmBtn;
#[derive(Component)]
pub(crate) struct CreateCancelBtn;

pub(crate) fn register(app: &mut App) {
    // Deliberately *not* gated on the hierarchy panel being active: the overlay
    // outlives the click that opened it, and a panel can be torn off, hidden or
    // swapped while it's up — which would otherwise strand it with dead buttons.
    app.add_systems(
        Update,
        (focus_name_field, create_overlay_buttons, create_overlay_reap),
    );
}

/// The "Attach" row for the entity context menu: one hover submenu listing every
/// kind. Returns the row to push into the menu's children.
/// `is_ui` narrows the list to what a canvas or widget can actually carry — see
/// [`CreateKind::on_ui`].
pub(crate) fn create_submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    target: Entity,
    is_ui: bool,
) -> Entity {
    let (row, content) = menu_submenu_styled(
        commands,
        fonts,
        "paperclip",
        &renzora::lang::t("hierarchy.context.attach"),
        (150, 190, 255),
    );
    let items: Vec<Entity> = CreateKind::ALL
        .iter()
        .filter(|k| !is_ui || k.on_ui())
        .map(|&kind| {
            menu_item_styled(
                commands,
                fonts,
                kind.icon(),
                &kind.label(),
                kind.color(),
                text_primary(),
                move |w: &mut World| open(w, kind, target),
            )
        })
        .collect();
    commands.entity(content).add_children(&items);
    row
}

/// Open the name + destination overlay for `kind`. Exclusive-world entry (menu
/// actions run as world closures) so it can read the project, pre-create the
/// conventional folder and build the tree in one shot.
fn open(world: &mut World, kind: CreateKind, target: Entity) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let Some(root) = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        // No project open — nothing to create *into*.
        if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
            toasts.warning(renzora::lang::t("hierarchy.create.no_project"));
        }
        return;
    };
    // Pre-create the conventional folder so the default destination is a real
    // row in the tree even on a project that has never had one.
    let default_dest = root.join(kind.default_dir());
    let _ = std::fs::create_dir_all(&default_dest);

    let entity_name = world
        .get::<Name>(target)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| format!("{target:?}"));

    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);

    let (overlay, content) = overlay_sized(
        &mut commands,
        &fonts,
        &format!("{} {}", renzora::lang::t("assets.new.header"), kind.label()),
        520.0,
        470.0,
        true,
    );

    let name_input = text_input(&mut commands, &fonts.ui, kind.stem(), kind.stem());
    let picker = folder_picker(&mut commands, &fonts, &root, &default_dest, PICKER_DEPTH);
    let mut kids = vec![
        field_row(
            &mut commands,
            &fonts,
            &renzora::lang::t("hierarchy.create.name"),
            name_input,
        ),
        section_label(&mut commands, &fonts, &renzora::lang::t("hierarchy.create.destination")),
        picker,
    ];

    let attach = kind.attachable().then(|| {
        let cb = checkbox(&mut commands, true);
        let row = check_row(
            &mut commands,
            &fonts,
            cb,
            &renzora::lang::t("hierarchy.create.attach").replace("{entity}", &entity_name),
        );
        kids.push(row);
        cb
    });

    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .id();
    // New Folder rides in the button row rather than under the tree — one row of
    // controls, not two. It floats at the row's left edge (absolute, out of
    // flow), so Cancel and Create lay out untouched.
    let new_folder = folder_new_button(&mut commands, &fonts, picker);
    let cancel = button(&mut commands, &fonts.ui, &renzora::lang::t("common.cancel"));
    commands.entity(cancel).insert(CreateCancelBtn);
    let confirm = button(&mut commands, &fonts.ui, &renzora::lang::t("hierarchy.create.confirm"));
    commands.entity(confirm).insert(CreateConfirmBtn);
    commands.entity(buttons).add_children(&[new_folder, cancel, confirm]);
    kids.push(buttons);

    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(14.0)),
                ..default()
            },
            // Enter in the name field = Create. Typing a name then reaching for
            // the mouse is the one interaction this overlay would otherwise
            // force on every single use.
            EmberForm { submit: confirm },
        ))
        .id();
    commands.entity(body).add_children(&kids);
    commands.entity(content).add_child(body);

    queue.apply(world);
    world.insert_resource(PendingCreate {
        kind,
        target,
        overlay,
        name_input,
        attach,
        ticks: 0,
    });
}

/// Focus the name field with its default selected, so the overlay is "type the
/// name, press Enter" with no click first.
///
/// Deliberately a tick late. The overlay is spawned from a menu click, and
/// ember's `text_input_focus` blurs every input on any left press that didn't
/// land *on* an input — our field doesn't exist yet when that press is read, so
/// focusing on the opening frame would be undone by whichever order the two
/// systems happen to run in. The next tick has no press to blur against.
fn focus_name_field(pending: Option<ResMut<PendingCreate>>, mut inputs: Query<&mut EmberTextInput>) {
    let Some(mut pending) = pending else { return };
    if pending.ticks > 1 {
        return;
    }
    pending.ticks += 1;
    if pending.ticks != 2 {
        return;
    }
    if let Ok(mut input) = inputs.get_mut(pending.name_input) {
        input.focused = true;
        // Select-all, so the first keystroke replaces the default name rather
        // than prepending to it.
        input.select_all = true;
        input.caret_index = input.value.chars().count();
    }
}

/// Confirm → write the file (and attach it); cancel → just close.
fn create_overlay_buttons(
    confirm: Query<&Interaction, (With<CreateConfirmBtn>, Changed<Interaction>)>,
    cancel: Query<&Interaction, (With<CreateCancelBtn>, Changed<Interaction>)>,
    pending: Option<Res<PendingCreate>>,
    inputs: Query<&EmberTextInput>,
    checks: Query<&Bound<bool>>,
    pick: Res<FolderPick>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut toasts: Option<ResMut<renzora_ui::Toasts>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };

    if cancel.iter().any(|i| *i == Interaction::Pressed) {
        commands.entity(pending.overlay).despawn();
        commands.remove_resource::<PendingCreate>();
        return;
    }
    if !confirm.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }

    let kind = pending.kind;
    let typed = inputs
        .get(pending.name_input)
        .map(|i| i.value.trim().to_string())
        .unwrap_or_default();
    let stem = if typed.is_empty() { kind.stem().to_string() } else { typed };
    let Some(root) = project.as_ref().map(|p| p.path.clone()) else {
        return;
    };
    let dir = pick
        .path()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(kind.default_dir()));

    commands.entity(pending.overlay).despawn();

    let path = unique_path(&dir, &stem, kind.ext());
    if let Err(e) = std::fs::write(&path, kind.content()) {
        if let Some(toasts) = toasts.as_mut() {
            toasts.error(format!("{}: {e}", path.display()));
        }
        renzora::core::console_log::console_error("Assets", format!("Create failed: {} — {e}", path.display()));
        commands.remove_resource::<PendingCreate>();
        return;
    }
    renzora::core::console_log::console_info("Assets", format!("Created {}", path.display()));

    // Attach only when the kind supports it *and* the checkbox is still ticked.
    let attached = kind.attachable()
        && pending
            .attach
            .and_then(|cb| checks.get(cb).ok())
            .is_none_or(|b| b.0);
    if attached {
        let rel = PathBuf::from(
            path.strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
        );
        let target = pending.target;
        commands.queue(move |w: &mut World| {
            let Ok(mut em) = w.get_entity_mut(target) else {
                // The entity was deleted while the overlay was up — the file is
                // still written, there's just nothing left to attach it to.
                return;
            };
            match em.get_mut::<ScriptComponent>() {
                Some(mut sc) => {
                    sc.add_file_script(rel);
                }
                None => {
                    em.insert(ScriptComponent::from_file(rel));
                }
            }
        });
    }

    if let Some(toasts) = toasts.as_mut() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        toasts.success(if attached {
            renzora::lang::t("hierarchy.create.toast_attached").replace("{name}", &name)
        } else {
            renzora::lang::t("hierarchy.create.toast_created").replace("{name}", &name)
        });
    }
    commands.remove_resource::<PendingCreate>();
}

/// Drop the pending state when the overlay goes away by any route ember owns
/// (Escape, backdrop click, the X) — without this the next Create would inherit
/// a stale name field and target entity.
fn create_overlay_reap(
    pending: Option<Res<PendingCreate>>,
    overlays: Query<Entity, With<Overlay>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    if overlays.get(pending.overlay).is_err() {
        commands.remove_resource::<PendingCreate>();
    }
}

/// `<folder>/<stem>.<ext>`, suffixed ` 2`, ` 3`… until it doesn't exist. Never
/// clobber: the name field is pre-filled with a default, so hitting Create twice
/// is an easy accident.
fn unique_path(folder: &Path, stem: &str, ext: &str) -> PathBuf {
    // A typed name that already carries the right extension shouldn't get a
    // second one ("player.lua" → "player.lua", not "player.lua.lua").
    let stem = stem
        .strip_suffix(&format!(".{ext}"))
        .unwrap_or(stem)
        .to_string();
    let candidate = folder.join(format!("{stem}.{ext}"));
    if !candidate.exists() {
        return candidate;
    }
    for n in 2..1000 {
        let cand = folder.join(format!("{stem} {n}.{ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    candidate
}

// ── Small overlay helpers ────────────────────────────────────────────────────

/// A labelled field: fixed-width label on the left, the widget filling the rest.
fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, field: Entity) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(70.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    // Wrap rather than re-inserting a `Node` on the field: the widget's own node
    // carries its padding, border and clip, and overwriting it would strip them.
    let grow = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            ..default()
        })
        .id();
    commands.entity(grow).add_child(field);
    commands.entity(row).add_children(&[l, grow]);
    row
}

/// A checkbox followed by its label.
fn check_row(commands: &mut Commands, fonts: &EmberFonts, cb: Entity, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();
    let l = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(row).add_children(&[cb, l]);
    row
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "folder-open", text_muted(), 11.0);
    let label = commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_children(&[icon, label]);
    row
}
