use crate::{adaptor::LoadContextAdaptor, data::HtmlTemplate, error::ParseError, parse::parse_template};
use bevy::{
    asset::{io::Reader, AssetLoader},
    prelude::*,
};

pub struct LoaderPlugin;
impl Plugin for LoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<HtmlTemplate>();
        app.init_asset_loader::<HtmlAssetLoader>();
    }
}

#[derive(Default, TypePath)]
pub struct HtmlAssetLoader;
impl AssetLoader for HtmlAssetLoader {
    type Asset = HtmlTemplate;
    type Settings = ();
    type Error = ParseError;

    async fn load<'a>(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'a>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|err| ParseError::FailedToRead(err.to_string()))?;
        let mut adapter = LoadContextAdaptor { load_context };
        match parse_template::<crate::error::VerboseHtmlError>(&bytes, &mut adapter) {
            Ok((_, template)) => Ok(template),
            Err(err) => match err {
                nom::Err::Incomplete(_) => Err(ParseError::Incomplete),
                nom::Err::Error(err) | nom::Err::Failure(err) => {
                    let file_path = load_context.path().to_string();
                    Err(ParseError::Nom(err.format(&bytes, &file_path)))
                }
            },
        }
    }

    fn extensions(&self) -> &[&str] {
        &["html", "xml"]
    }
}
