use crate::adaptor::AssetLoadAdaptor;
use crate::animation::{AnimationDirection, Atlas};
use crate::prelude::*;
use crate::util::{SlotId, SlotMap};
use bevy::ecs::system::EntityCommands;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;

#[derive(Debug, Default, Reflect)]
#[reflect]
pub enum NodeType {
    #[default]
    Node,
    Image,
    Text,
    Button,
    Slot,
    Template,
    Property,
    Custom(String),
}

/// a single nodes data
#[derive(Debug, Default, Reflect)]
#[reflect]
pub struct XNode {
    pub uuid: u64,
    pub src: Option<String>,
    pub styles: Vec<StyleAttr>,
    pub target: Option<String>,
    pub watch: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub uncompiled: Vec<AttrTokens>,
    pub tags: HashMap<String, String>,
    pub defs: HashMap<String, String>,
    pub event_listener: Vec<Action>,
    pub content_id: SlotId,
    pub node_type: NodeType,
    #[reflect(ignore)]
    pub children: Vec<XNode>,
}

/// holds a parsed template
/// can be build as UI.
#[derive(Debug, Asset, Reflect)]
#[reflect]
pub struct HtmlTemplate {
    pub name: Option<String>,
    pub properties: HashMap<String, String>,
    pub root: Vec<XNode>,
    pub content: SlotMap<String>,
}

/// any valid attribute that can be found
/// on nodes.
#[derive(Debug, Clone, Reflect)]
#[reflect]
pub enum Attribute {
    Style(StyleAttr),
    PropertyDefinition(String, String),
    Name(String),
    Uncompiled(AttrTokens),
    Action(Action),
    Path(String),
    Target(String),
    Id(String),
    Watch(String),
    Tag(String, String),
}

/// raw attribute
#[derive(Debug, Reflect, PartialEq, Clone)]
#[reflect]
pub struct AttrTokens {
    pub prefix: Option<String>,
    pub ident: String,
    pub key: String,
}

impl AttrTokens {
    pub fn compile(&self, props: &TemplateProperties, loader: &mut impl AssetLoadAdaptor) -> Option<Attribute> {
        let Some(prop_val) = props.get(&self.key) else {
            return None;
        };

        let (_, attr) = match crate::parse::attribute_from_parts::<nom::error::Error<&[u8]>>(
            self.prefix.as_ref().map(|s| s.as_bytes()),
            self.ident.as_bytes(),
            prop_val.as_bytes(),
            loader
        ) {
            Ok(val) => val,
            Err(_) => (
                "".as_bytes(),
                Attribute::PropertyDefinition(self.ident.to_owned(), prop_val.to_owned()),
            ),
        };

        // recursive compile, what could go wrong
        if let Attribute::Uncompiled(attr) = attr {
            return attr.compile(props, loader);
        };

        Some(attr)
    }
}

#[derive(Debug, Reflect, PartialEq, Clone)]
#[reflect]
pub enum Action {
    OnPress(Vec<String>),
    OnEnter(Vec<String>),
    OnExit(Vec<String>),
    OnSpawn(Vec<String>),
    OnChange(Vec<String>),
}

impl Action {
    pub fn self_insert(self, mut cmd: EntityCommands) {
        match self {
            Action::OnPress(fn_id) => {
                cmd.insert(crate::prelude::OnUiPress(fn_id));
            }
            Action::OnEnter(fn_id) => {
                cmd.insert(crate::prelude::OnUiEnter(fn_id));
            }
            Action::OnExit(fn_id) => {
                cmd.insert(crate::prelude::OnUiExit(fn_id));
            }
            Action::OnSpawn(fn_id) => {
                cmd.insert(crate::prelude::OnUiSpawn(fn_id));
            }
            Action::OnChange(fn_id) => {
                cmd.insert(crate::prelude::OnUiChange(fn_id));
            }
        }
    }
}

#[derive(Debug, Clone, Reflect)]
#[reflect]
pub enum FontReference {
    Handle(Handle<Font>),
    Path(String),
}

#[derive(Debug, Clone, Reflect)]
#[reflect]
pub enum StyleAttr {
    Display(Display),
    Position(PositionType),
    Overflow(Overflow),
    OverflowClipMargin(OverflowClipMargin),
    FrameTime(Val),
    Left(Val),
    Right(Val),
    Top(Val),
    Bottom(Val),
    Width(Val),
    Height(Val),
    MinWidth(Val),
    MinHeight(Val),
    MaxWidth(Val),
    MaxHeight(Val),
    AspectRatio(f32),
    AlignItems(AlignItems),
    JustifyItems(JustifyItems),
    AlignSelf(AlignSelf),
    JustifySelf(JustifySelf),
    AlignContent(AlignContent),
    JustifyContent(JustifyContent),
    Margin(UiRect),
    Padding(UiRect),
    Zindex(ZIndex),
    GlobalZIndex(GlobalZIndex),

    // ------------
    // border
    Border(UiRect),
    BorderColor(Color),
    BorderRadius(UiRect),
    Outline(Outline),

    // ------------
    // flex
    FlexDirection(FlexDirection),
    FlexWrap(FlexWrap),
    FlexGrow(f32),
    FlexShrink(f32),
    FlexBasis(Val),
    RowGap(Val),
    ColumnGap(Val),

    // -----------
    // grid
    GridAutoFlow(GridAutoFlow),
    GridTemplateRows(Vec<RepeatedGridTrack>),
    GridTemplateColumns(Vec<RepeatedGridTrack>),
    GridAutoRows(Vec<GridTrack>),
    GridAutoColumns(Vec<GridTrack>),
    GridRow(GridPlacement),
    GridColumn(GridPlacement),

    // -----
    // font
    FontSize(f32),
    Font(FontReference),
    FontColor(Color),
    TextLayout(TextLayout),

    // -----
    // color
    Background(Color),
    ShadowColor(Color),
    ShadowOffset(Val, Val),
    ShadowSpread(Val),
    ShadowBlur(Val),
    TextShadow(TextShadow),

    // -----
    Hover(#[reflect(ignore)] Box<StyleAttr>),
    Pressed(#[reflect(ignore)] Box<StyleAttr>),
    Active(#[reflect(ignore)] Box<StyleAttr>),

    // -----
    // animations
    Delay(f32),
    Easing(EaseFunction),
    Atlas(Option<Atlas>),
    Duration(f32),
    Iterations(i64),
    Direction(AnimationDirection),
    FPS(i64),
    Frames(Vec<i64>),

    // -----
    // image
    ImageColor(Color),
    ImageScaleMode(NodeImageMode),
    ImageRegion(Rect),

    // -----
    // picking
    #[cfg(feature = "picking")]
    Pickable((bool, bool)),
}

impl Default for StyleAttr {
    fn default() -> Self {
        StyleAttr::Display(Display::None)
    }
}
