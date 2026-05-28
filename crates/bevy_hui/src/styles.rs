use crate::{
    animation::{AnimationDirection, Atlas},
    build::InteractionObverser,
    data::{FontReference, StyleAttr},
};
use bevy::{
    ecs::{query::QueryEntityError, system::SystemParam},
    prelude::*,
    ui::widget::NodeImageMode,
};
#[cfg(feature = "picking")]
use bevy_picking::Pickable;
use std::time::Duration;

pub struct TransitionPlugin;
impl Plugin for TransitionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (continues_interaction_checking, update_node_style));
        app.register_type::<PressedTimer>();
        app.register_type::<HoverTimer>();
        app.register_type::<InteractionTimer>();
        app.register_type::<ComputedStyle>();
        app.register_type::<HtmlStyle>();
    }
}

/// interpolation timer for
/// transitions
#[derive(Component, Clone, Default, Reflect)]
#[reflect]
pub struct InteractionTimer {
    elapsed: Duration,
    max: Duration,
}

/// add this component to enable
/// all active styles
#[derive(Component)]
pub struct UiActive;

impl InteractionTimer {
    pub fn new(max: Duration) -> Self {
        Self {
            elapsed: Duration::ZERO,
            max,
        }
    }

    pub fn fraction(&self) -> f32 {
        self.elapsed.div_duration_f32(self.max)
    }

    pub fn forward(&mut self, delta: Duration) {
        self.elapsed = self
            .elapsed
            .checked_add(delta)
            .map(|d| d.min(self.max))
            .unwrap_or(self.elapsed);
    }

    pub fn backward(&mut self, delta: Duration) {
        self.elapsed = self.elapsed.checked_sub(delta).unwrap_or(Duration::ZERO);
    }
}

fn continues_interaction_checking(
    interactions: Query<(Entity, &Interaction), With<HtmlStyle>>,
    mut hovers: Query<&mut HoverTimer>,
    mut presseds: Query<&mut PressedTimer>,
    observer: Query<&InteractionObverser>,
    time: Res<Time<Real>>,
) {
    interactions.iter().for_each(|(entity, interaction)| {
        let subs = observer
            .get(entity)
            .map(|obs| obs.iter())
            .unwrap_or_default()
            .chain(std::iter::once(&entity));

        match interaction {
            Interaction::Pressed => {
                // ++ pressed ++ hover
                subs.for_each(|sub| {
                    if let (Ok(mut htimer), Ok(mut ptimer)) =
                        (hovers.get_mut(*sub), presseds.get_mut(*sub))
                    {
                        ptimer.forward(time.delta());
                        htimer.forward(time.delta());
                    } else {
                        warn!("non interacting node obsering `{sub}`")
                    }
                });
            }
            Interaction::Hovered => {
                // ++ hover -- pressed
                subs.for_each(|sub| {
                    if let (Ok(mut htimer), Ok(mut ptimer)) =
                        (hovers.get_mut(*sub), presseds.get_mut(*sub))
                    {
                        ptimer.backward(time.delta());
                        htimer.forward(time.delta());
                    } else {
                        warn!("non interacting node obsering `{sub}`")
                    }
                });
            }
            Interaction::None => {
                // -- hover --pressed
                subs.for_each(|sub| {
                    if let (Ok(mut htimer), Ok(mut ptimer)) =
                        (hovers.get_mut(*sub), presseds.get_mut(*sub))
                    {
                        ptimer.backward(time.delta());
                        htimer.backward(time.delta());
                    } else {
                        warn!("non interacting node obsering `{sub}`")
                    }
                });
            }
        };
    });
}

#[derive(SystemParam)]
pub struct UiStyleQuery<'w, 's> {
    pub server: Res<'w, AssetServer>,
    pub node: Query<'w, 's, &'static mut Node>,
    pub image: Query<'w, 's, &'static mut ImageNode>,
    pub text_fonts: Query<'w, 's, &'static mut TextFont>,
    pub text_colors: Query<'w, 's, &'static mut TextColor>,
    pub text_layouts: Query<'w, 's, &'static mut TextLayout>,
    pub text_shadows: Query<'w, 's, &'static mut TextShadow>,
    pub background: Query<'w, 's, &'static mut BackgroundColor>,
    pub border_color: Query<'w, 's, &'static mut BorderColor>,
    pub shadow: Query<'w, 's, &'static mut BoxShadow>,
    pub outline: Query<'w, 's, &'static mut Outline>,
}

impl<'w, 's> UiStyleQuery<'w, 's> {
    pub fn apply_computed(
        &mut self,
        entity: Entity,
        computed: &mut ComputedStyle,
        server: &AssetServer,
    ) {
        _ = self.node.get_mut(entity).map(|mut node| {
            node.clone_from(&computed.node);
        });

        _ = self.image.get_mut(entity).map(|mut image| {
            image.color = computed.image_color;
        });

        _ = self.text_fonts.get_mut(entity).map(|mut font| {
            font.font_size = computed.font_size;
            if let Some(h) = computed.font.as_ref() {
                let (handle, update_style) = match h {
                    FontReference::Handle(handle) => (handle.clone(), false),
                    FontReference::Path(path) => (server.load(path), true),
                };
                if update_style {
                    // update the computed style with the new font handle
                    // this is needed to prevent the font from being reloaded
                    // on every frame
                    computed.font = Some(FontReference::Handle(handle.clone()));
                }
                if font.font != handle {
                    font.font = handle;
                }
            }
        });

        _ = self.text_colors.get_mut(entity).map(|mut color| {
            **color = computed.font_color;
        });

        _ = self.background.get_mut(entity).map(|mut background| {
            background.0 = computed.background;
        });

        _ = self.node.get_mut(entity).map(|mut node| {
            node.border_radius.top_left = computed.border_radius.top;
            node.border_radius.top_right = computed.border_radius.right;
            node.border_radius.bottom_right = computed.border_radius.bottom;
            node.border_radius.bottom_left = computed.border_radius.left;
        });

        if let Some(computed_shadow) = computed.text_shadow.as_ref() {
            _ = self.text_shadows.get_mut(entity).map(|mut shadow| {
                shadow.color = computed_shadow.color;
                shadow.offset = computed_shadow.offset;
            });
        }

        if let Some(computed_shadow) = computed.shadow.as_ref() {
            _ = self.shadow.get_mut(entity).map(|mut shadow| {
                *shadow = computed_shadow.clone();
            });
        }

        _ = self.border_color.get_mut(entity).map(|mut color| {
            color.right = computed.border_color;
            color.left = computed.border_color;
            color.top = computed.border_color;
            color.bottom = computed.border_color;
        });
    }

    pub fn apply_interpolated(
        &mut self,
        entity: Entity,
        ratio: f32,
        computed: &ComputedStyle,
        attr: &StyleAttr,
    ) -> Result<(), QueryEntityError> {
        let mut style = self.node.get_mut(entity)?;
        match attr {
            StyleAttr::Display(display) => style.display = *display,
            StyleAttr::Position(position_type) => style.position_type = *position_type,
            StyleAttr::Overflow(overflow) => style.overflow = *overflow,
            StyleAttr::Left(val) => style.left = lerp_val(&computed.node.left, val, ratio),
            StyleAttr::Right(val) => style.right = lerp_val(&computed.node.right, val, ratio),
            StyleAttr::Top(val) => style.top = lerp_val(&computed.node.top, val, ratio),
            StyleAttr::Bottom(val) => style.bottom = lerp_val(&computed.node.bottom, val, ratio),
            StyleAttr::Width(val) => style.width = lerp_val(&computed.node.width, val, ratio),
            StyleAttr::Height(val) => style.height = lerp_val(&computed.node.height, val, ratio),
            StyleAttr::MinWidth(val) => {
                style.min_width = lerp_val(&computed.node.min_width, val, ratio)
            }
            StyleAttr::MinHeight(val) => {
                style.min_height = lerp_val(&computed.node.min_height, val, ratio)
            }
            StyleAttr::MaxWidth(val) => {
                style.max_width = lerp_val(&computed.node.max_width, val, ratio)
            }
            StyleAttr::MaxHeight(val) => {
                style.max_height = lerp_val(&computed.node.max_height, val, ratio)
            }
            StyleAttr::AspectRatio(f) => {
                style.aspect_ratio = computed.node.aspect_ratio.map(|a| a.lerp(*f, ratio))
            }
            StyleAttr::AlignItems(align_items) => style.align_items = *align_items,
            StyleAttr::JustifyItems(justify_items) => style.justify_items = *justify_items,
            StyleAttr::AlignSelf(align_self) => style.align_self = *align_self,
            StyleAttr::JustifySelf(justify_self) => style.justify_self = *justify_self,
            StyleAttr::AlignContent(align_content) => style.align_content = *align_content,
            StyleAttr::JustifyContent(justify_content) => style.justify_content = *justify_content,
            StyleAttr::Margin(ui_rect) => {
                style.margin = lerp_rect(&computed.node.margin, ui_rect, ratio)
            }
            StyleAttr::Padding(ui_rect) => {
                style.padding = lerp_rect(&computed.node.padding, ui_rect, ratio)
            }
            StyleAttr::Outline(outline) => {
                if let Some(regular) = &computed.outline.as_ref() {
                    _ = self.outline.get_mut(entity).map(|mut line| {
                        line.width = lerp_val(&regular.width, &outline.width, ratio);
                        line.offset = lerp_val(&regular.offset, &outline.offset, ratio);
                        line.color = lerp_color(&regular.color, &outline.color, ratio);
                    });
                }
            }
            StyleAttr::ImageColor(color) => {
                _ = self
                    .image
                    .get_mut(entity)
                    .map(|mut image| image.color = lerp_color(&computed.image_color, color, ratio));
            }
            StyleAttr::Border(ui_rect) => {
                style.border = lerp_rect(&computed.node.border, ui_rect, ratio)
            }
            StyleAttr::BorderColor(color) => {
                _ = self.border_color.get_mut(entity).map(|mut bcolor| {
                    let color = lerp_color(&computed.border_color, color, ratio);
                    bcolor.right = color;
                    bcolor.left = color;
                    bcolor.top = color;
                    bcolor.bottom = color;
                });
            }
            StyleAttr::BorderRadius(ui_rect) => {
                _ = self.node.get_mut(entity).map(|mut node| {
                    node.border_radius.top_left =
                        lerp_val(&computed.border_radius.top, &ui_rect.top, ratio);
                    node.border_radius.top_right =
                        lerp_val(&computed.border_radius.right, &ui_rect.right, ratio);
                    node.border_radius.bottom_right =
                        lerp_val(&computed.border_radius.bottom, &ui_rect.bottom, ratio);
                    node.border_radius.bottom_left =
                        lerp_val(&computed.border_radius.left, &ui_rect.left, ratio);
                });
            }
            StyleAttr::FlexDirection(flex_direction) => style.flex_direction = *flex_direction,
            StyleAttr::FlexWrap(flex_wrap) => style.flex_wrap = *flex_wrap,
            StyleAttr::FlexGrow(g) => style.flex_grow = computed.node.flex_grow.lerp(*g, ratio),
            StyleAttr::FlexShrink(s) => {
                style.flex_shrink = computed.node.flex_shrink.lerp(*s, ratio)
            }
            StyleAttr::FlexBasis(val) => {
                style.flex_basis = lerp_val(&computed.node.flex_basis, val, ratio)
            }
            StyleAttr::RowGap(val) => style.row_gap = lerp_val(&computed.node.row_gap, val, ratio),
            StyleAttr::ColumnGap(val) => {
                style.column_gap = lerp_val(&computed.node.column_gap, val, ratio)
            }
            StyleAttr::GridAutoFlow(grid_auto_flow) => style.grid_auto_flow = *grid_auto_flow,
            StyleAttr::GridTemplateRows(vec) => style.grid_template_rows = vec.clone(),
            StyleAttr::GridTemplateColumns(vec) => style.grid_template_columns = vec.clone(),
            StyleAttr::GridAutoRows(vec) => style.grid_auto_rows = vec.clone(),
            StyleAttr::GridAutoColumns(vec) => style.grid_auto_columns = vec.clone(),
            StyleAttr::GridRow(grid_placement) => style.grid_row = *grid_placement,
            StyleAttr::GridColumn(grid_placement) => style.grid_column = *grid_placement,
            StyleAttr::Background(color) => {
                _ = self
                    .background
                    .get_mut(entity)
                    .map(|mut bg| bg.0 = lerp_color(&computed.background, color, ratio));
            }
            StyleAttr::FontColor(color) => {
                _ = self.text_colors.get_mut(entity).map(|mut tc| {
                    **tc = lerp_color(&computed.font_color, color, ratio);
                });
            }
            StyleAttr::TextLayout(text_layout) => {
                _ = self
                    .text_layouts
                    .get_mut(entity)
                    .map(|mut tl| *tl = *text_layout)
            }
            StyleAttr::FontSize(s) => {
                _ = self.text_fonts.get_mut(entity).map(|mut txt| {
                    txt.font_size = computed.font_size.lerp(*s, ratio);
                });
            }
            StyleAttr::Font(h) => {
                _ = self.text_fonts.get_mut(entity).map(|mut txt| {
                    txt.font = match h {
                        FontReference::Handle(handle) => {
                            handle.clone()
                        }
                        FontReference::Path(path) => {
                            warn!("Font path `{path}` is being loaded during a transition, this is not recommended!");
                            self.server.load(path)
                        }
                    };
                });
            }
            StyleAttr::ShadowColor(color) => {
                if let Some(computed_shadow) = computed.shadow.as_ref() {
                    _ = self.shadow.get_mut(entity).map(|mut shadow| {
                        shadow[0].color = lerp_color(&computed_shadow[0].color, color, ratio)
                    });
                }
            }
            StyleAttr::TextShadow(shadow) => {
                if let Some(computed_shadow) = computed.text_shadow.as_ref() {
                    _ = self.text_shadows.get_mut(entity).map(|mut s| {
                        s.offset = computed_shadow.offset.lerp(shadow.offset, ratio);
                        s.color = lerp_color(&computed_shadow.color, &shadow.color, ratio);
                    });
                }
            }
            StyleAttr::ShadowOffset(x, y) => {
                if let Some(computed_shadow) = computed.shadow.as_ref() {
                    _ = self.shadow.get_mut(entity).map(|mut shadow| {
                        shadow[0].x_offset = lerp_val(&computed_shadow[0].x_offset, x, ratio);
                        shadow[0].y_offset = lerp_val(&computed_shadow[0].y_offset, y, ratio);
                    });
                }
            }
            StyleAttr::ShadowBlur(blur) => {
                if let Some(computed_shadow) = computed.shadow.as_ref() {
                    _ = self.shadow.get_mut(entity).map(|mut shadow| {
                        shadow[0].blur_radius =
                            lerp_val(&computed_shadow[0].blur_radius, blur, ratio);
                    });
                }
            }
            StyleAttr::ShadowSpread(spread) => {
                if let Some(computed_shadow) = computed.shadow.as_ref() {
                    _ = self.shadow.get_mut(entity).map(|mut shadow| {
                        shadow[0].spread_radius =
                            lerp_val(&computed_shadow[0].spread_radius, spread, ratio);
                    });
                }
            }
            _ => (),
        }

        Ok(())
    }
}

fn update_node_style(
    mut nodes: Query<(Entity, &mut HtmlStyle, Has<UiActive>)>,
    mut ui_style: UiStyleQuery,
    hover_timer: Query<&HoverTimer>,
    press_timer: Query<&PressedTimer>,
    server: Res<AssetServer>,
) {
    for (entity, mut html_style, is_active) in nodes.iter_mut() {
        ui_style.apply_computed(entity, &mut html_style.computed, &server);

        let hover_ratio = hover_timer
            .get(entity)
            .map(|t| t.fraction())
            .unwrap_or_default();

        let hover_ratio = html_style
            .computed
            .easing
            .map(|ease| EasingCurve::new(0., 1., ease).sample(hover_ratio))
            .flatten()
            .unwrap_or(hover_ratio);

        for hover_style in html_style.hover.iter() {
            ui_style
                .apply_interpolated(entity, hover_ratio, &html_style.computed, hover_style)
                .expect("node has no style, impossible");
        }

        let press_ratio = press_timer
            .get(entity)
            .map(|t| t.fraction())
            .unwrap_or_default();

        let press_ratio = html_style
            .computed
            .easing
            .map(|ease| EasingCurve::new(0., 1., ease).sample(press_ratio))
            .flatten()
            .unwrap_or(press_ratio);

        for press_style in html_style.pressed.iter() {
            ui_style
                .apply_interpolated(entity, press_ratio, &html_style.computed, press_style)
                .expect("node has no style, impossible");
        }

        let active_ratio = is_active.then_some(1.).unwrap_or_default();
        for active_style in html_style.active.iter() {
            ui_style
                .apply_interpolated(entity, active_ratio, &html_style.computed, active_style)
                .expect("node has no style, impossible");
        }
    }
}

#[derive(Component, Reflect, Clone, Default, Deref, DerefMut)]
#[reflect]
pub struct PressedTimer(InteractionTimer);

impl PressedTimer {
    pub fn new(d: Duration) -> Self {
        Self(InteractionTimer::new(d))
    }
}

#[derive(Component, Default, Clone, Reflect, Deref, DerefMut)]
#[reflect]
pub struct HoverTimer(InteractionTimer);

impl HoverTimer {
    pub fn new(d: Duration) -> Self {
        Self(InteractionTimer::new(d))
    }
}

#[derive(Debug, Reflect, Clone)]
#[reflect]
pub struct ComputedStyle {
    pub node: Node,
    pub border_color: Color,
    pub border_radius: UiRect,
    pub image_color: Color,
    pub image_mode: Option<NodeImageMode>,
    pub image_region: Option<Rect>,
    pub shadow: Option<BoxShadow>,
    pub text_shadow: Option<TextShadow>,
    pub background: Color,
    pub outline: Option<Outline>,
    pub font: Option<FontReference>,
    pub text_layout: Option<TextLayout>,
    pub font_size: f32,
    pub font_color: Color,
    pub atlas: Option<Atlas>,
    pub delay: f32,
    pub duration: f32,
    pub iterations: i64,
    pub fps: i64,
    pub frames: Vec<i64>,
    pub direction: AnimationDirection,
    pub easing: Option<EaseFunction>,
    pub zindex: Option<ZIndex>,
    pub global_zindex: Option<GlobalZIndex>,
    #[cfg(feature = "picking")]
    pub pickable: Option<Pickable>,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            node: Node::default(),
            image_color: Color::WHITE,
            border_color: Color::NONE,
            border_radius: UiRect::default(),
            background: Color::NONE,
            image_mode: None,
            shadow: None,
            image_region: None,
            outline: None,
            text_shadow: None,
            font: Default::default(),
            font_size: 12.,
            font_color: Color::WHITE,
            text_layout: None,
            atlas: None,
            delay: 0.,
            duration: 0.,
            fps: 1,
            frames: Vec::new(),
            iterations: -1,
            direction: AnimationDirection::Forward,
            easing: Some(EaseFunction::Linear),
            zindex: None,
            global_zindex: None,
            #[cfg(feature = "picking")]
            pickable: None,
        }
    }
}

/// this components holds all relevant style
/// attributes.
#[derive(Component, Default, Clone, Debug, Reflect)]
#[reflect]
pub struct HtmlStyle {
    pub computed: ComputedStyle,
    pub hover: Vec<StyleAttr>,
    pub pressed: Vec<StyleAttr>,
    pub active: Vec<StyleAttr>,
}

impl From<Vec<StyleAttr>> for HtmlStyle {
    fn from(mut styles: Vec<StyleAttr>) -> Self {
        let mut out = HtmlStyle::default();
        for style in styles.drain(..) {
            out.add_style_attr(style, None);
        }
        out
    }
}

impl HtmlStyle {
    pub fn add_style_attr(&mut self, attr: StyleAttr, server: Option<&AssetServer>) {
        match attr {
            StyleAttr::Hover(style) => {
                let style = *style;
                match self
                    .hover
                    .iter()
                    .position(|s| std::mem::discriminant(s) == std::mem::discriminant(&style))
                {
                    Some(index) => self.hover.insert(index, style),
                    None => self.hover.push(style),
                }
            }
            StyleAttr::Pressed(style) => {
                let style = *style;
                match self
                    .pressed
                    .iter()
                    .position(|s| std::mem::discriminant(s) == std::mem::discriminant(&style))
                {
                    Some(index) => self.pressed.insert(index, style),
                    None => self.pressed.push(style),
                }
            }
            StyleAttr::Active(style) => {
                let style = *style;
                match self
                    .active
                    .iter()
                    .position(|s| std::mem::discriminant(s) == std::mem::discriminant(&style))
                {
                    Some(index) => self.active.insert(index, style),
                    None => self.active.push(style),
                }
            }
            StyleAttr::Display(display) => self.computed.node.display = display,
            StyleAttr::Position(position_type) => self.computed.node.position_type = position_type,
            StyleAttr::Overflow(overflow) => self.computed.node.overflow = overflow,
            StyleAttr::OverflowClipMargin(overflow_clip) => {
                self.computed.node.overflow_clip_margin = overflow_clip
            }
            StyleAttr::Left(val) => self.computed.node.left = val,
            StyleAttr::Right(val) => self.computed.node.right = val,
            StyleAttr::Top(val) => self.computed.node.top = val,
            StyleAttr::Bottom(val) => self.computed.node.bottom = val,
            StyleAttr::Width(val) => self.computed.node.width = val,
            StyleAttr::Height(val) => self.computed.node.height = val,
            StyleAttr::MinWidth(val) => self.computed.node.min_width = val,
            StyleAttr::MinHeight(val) => self.computed.node.min_height = val,
            StyleAttr::MaxWidth(val) => self.computed.node.max_width = val,
            StyleAttr::MaxHeight(val) => self.computed.node.max_height = val,
            StyleAttr::AspectRatio(f) => self.computed.node.aspect_ratio = Some(f),
            StyleAttr::AlignItems(align_items) => self.computed.node.align_items = align_items,
            StyleAttr::JustifyItems(justify_items) => {
                self.computed.node.justify_items = justify_items
            }
            StyleAttr::AlignSelf(align_self) => self.computed.node.align_self = align_self,
            StyleAttr::JustifySelf(justify_self) => self.computed.node.justify_self = justify_self,
            StyleAttr::AlignContent(align_content) => {
                self.computed.node.align_content = align_content
            }
            StyleAttr::JustifyContent(justify_content) => {
                self.computed.node.justify_content = justify_content
            }
            StyleAttr::ImageColor(color) => self.computed.image_color = color,
            StyleAttr::Zindex(index) => self.computed.zindex = Some(index),
            StyleAttr::GlobalZIndex(index) => self.computed.global_zindex = Some(index),
            StyleAttr::Margin(ui_rect) => self.computed.node.margin = ui_rect,
            StyleAttr::Padding(ui_rect) => self.computed.node.padding = ui_rect,
            StyleAttr::Border(ui_rect) => self.computed.node.border = ui_rect,
            StyleAttr::BorderColor(color) => self.computed.border_color = color,
            StyleAttr::BorderRadius(ui_rect) => self.computed.border_radius = ui_rect,
            StyleAttr::FlexDirection(flex_direction) => {
                self.computed.node.flex_direction = flex_direction
            }
            StyleAttr::FlexWrap(flex_wrap) => self.computed.node.flex_wrap = flex_wrap,
            StyleAttr::FlexGrow(f) => self.computed.node.flex_grow = f,
            StyleAttr::FlexShrink(f) => self.computed.node.flex_shrink = f,
            StyleAttr::FlexBasis(val) => self.computed.node.flex_basis = val,
            StyleAttr::RowGap(val) => self.computed.node.row_gap = val,
            StyleAttr::ColumnGap(val) => self.computed.node.column_gap = val,
            StyleAttr::GridAutoFlow(grid_auto_flow) => {
                self.computed.node.grid_auto_flow = grid_auto_flow
            }
            StyleAttr::GridTemplateRows(vec) => self.computed.node.grid_template_rows = vec,
            StyleAttr::GridTemplateColumns(vec) => self.computed.node.grid_template_columns = vec,
            StyleAttr::GridAutoRows(vec) => self.computed.node.grid_auto_rows = vec,
            StyleAttr::GridAutoColumns(vec) => self.computed.node.grid_auto_columns = vec,
            StyleAttr::GridRow(grid_placement) => self.computed.node.grid_row = grid_placement,
            StyleAttr::GridColumn(grid_placement) => {
                self.computed.node.grid_column = grid_placement
            }
            StyleAttr::FontSize(f) => self.computed.font_size = f,
            StyleAttr::FontColor(color) => self.computed.font_color = color,
            StyleAttr::TextLayout(text_layout) => self.computed.text_layout = Some(text_layout),
            StyleAttr::Background(color) => self.computed.background = color,
            StyleAttr::Atlas(f) => self.computed.atlas = f,
            StyleAttr::Delay(f) => self.computed.delay = f,
            StyleAttr::Duration(f) => self.computed.duration = f,
            StyleAttr::FPS(f) => self.computed.fps = f,
            StyleAttr::Iterations(f) => self.computed.iterations = f,
            StyleAttr::Direction(f) => self.computed.direction = f,
            StyleAttr::Frames(f) => self.computed.frames = f,
            StyleAttr::Easing(ease) => self.computed.easing = Some(ease),
            StyleAttr::ImageScaleMode(mode) => self.computed.image_mode = Some(mode),
            StyleAttr::ImageRegion(rect) => self.computed.image_region = Some(rect),
            StyleAttr::Outline(outline) => self.computed.outline = Some(outline),

            StyleAttr::ShadowSpread(spread_radius) => match self.computed.shadow.as_mut() {
                Some(shadow) => shadow[0].spread_radius = spread_radius,
                None => {
                    self.computed.shadow = Some(BoxShadow::new(
                        Color::default(),
                        Val::default(),
                        Val::default(),
                        spread_radius,
                        Val::default(),
                    ));
                }
            },
            StyleAttr::ShadowBlur(blur_radius) => match self.computed.shadow.as_mut() {
                Some(shadow) => shadow[0].blur_radius = blur_radius,
                None => {
                    self.computed.shadow = Some(BoxShadow::new(
                        Color::default(),
                        Val::default(),
                        Val::default(),
                        Val::default(),
                        blur_radius,
                    ));
                }
            },
            StyleAttr::ShadowColor(color) => match self.computed.shadow.as_mut() {
                Some(shadow) => shadow[0].color = color,
                None => {
                    self.computed.shadow = Some(BoxShadow::new(
                        color,
                        Val::default(),
                        Val::default(),
                        Val::default(),
                        Val::default(),
                    ));
                }
            },
            StyleAttr::TextShadow(shadow) => self.computed.text_shadow = Some(shadow),
            StyleAttr::ShadowOffset(x, y) => match self.computed.shadow.as_mut() {
                Some(shadow) => {
                    shadow[0].x_offset = x;
                    shadow[0].y_offset = y;
                }
                None => {
                    self.computed.shadow = Some(BoxShadow::new(
                        Color::default(),
                        x,
                        y,
                        Val::default(),
                        Val::default(),
                    ));
                }
            },
            #[cfg(feature = "picking")]
            StyleAttr::Pickable((should_block_lower, is_hoverable)) => {
                self.computed.pickable = Some(Pickable {
                    should_block_lower,
                    is_hoverable,
                })
            }
            StyleAttr::Font(font) => {
                self.computed.font = match (font, server) {
                    // opportunistically load the font if the asset server is available
                    (FontReference::Path(path), Some(server)) => {
                        Some(FontReference::Handle(server.load(path)))
                    }
                    (handle @ FontReference::Handle(..), _) => Some(handle),
                    (path @ FontReference::Path(..), None) => Some(path),
                }
            }
            _ => (),
        };
    }
}

fn lerp_color(start: &Color, end: &Color, ratio: f32) -> Color {
    let lin = start
        .to_linear()
        .to_vec4()
        .lerp(end.to_linear().to_vec4(), ratio);

    Color::LinearRgba(LinearRgba::from_vec4(lin))
}

fn lerp_rect(start: &UiRect, end: &UiRect, ratio: f32) -> UiRect {
    UiRect::new(
        lerp_val(&start.left, &end.left, ratio),
        lerp_val(&start.right, &end.right, ratio),
        lerp_val(&start.top, &end.top, ratio),
        lerp_val(&start.bottom, &end.bottom, ratio),
    )
}

fn lerp_val(start: &Val, end: &Val, ratio: f32) -> Val {
    match (start, end) {
        (Val::Percent(start), Val::Percent(end)) => {
            Val::Percent((end - start).mul_add(ratio, *start))
        }
        (Val::Px(start), Val::Px(end)) => Val::Px((end - start).mul_add(ratio, *start)),
        _ => *start,
    }
}
