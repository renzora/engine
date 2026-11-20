#[allow(unused_imports)]
use std::io::{BufReader, Cursor, ErrorKind};

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    reflect::TypePath,
};

use crate::{
    gaussian::formats::planar_3d::PlanarGaussian3d, gaussian::formats::planar_4d::PlanarGaussian4d,
    io::codec::CloudCodec,
};

#[derive(Default, TypePath)]
pub struct Gaussian3dLoader;

impl AssetLoader for Gaussian3dLoader {
    type Asset = PlanarGaussian3d;
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

        let extension = load_context
            .path()
            .path()
            .extension()
            .and_then(|ext| ext.to_str());

        match extension {
            Some("ply") => {
                #[cfg(feature = "io_ply")]
                {
                    let cursor = Cursor::new(bytes);
                    let mut f = BufReader::new(cursor);

                    Ok(crate::io::ply::parse_ply_3d(&mut f)?)
                }

                #[cfg(not(feature = "io_ply"))]
                {
                    Err(std::io::Error::other(
                        "ply support not enabled, enable with io_ply feature",
                    ))
                }
            }
            Some("gcloud") => {
                let cloud = PlanarGaussian3d::decode(bytes.as_slice());

                Ok(cloud)
            }
            _ => Err(std::io::Error::other("only .ply and .gcloud supported")),
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ply", "gcloud"]
    }
}

#[derive(Default, TypePath)]
pub struct Gaussian4dLoader;

impl AssetLoader for Gaussian4dLoader {
    type Asset = PlanarGaussian4d;
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

        let extension = load_context
            .path()
            .path()
            .extension()
            .and_then(|ext| ext.to_str());

        match extension {
            Some("ply4d") => {
                #[cfg(feature = "io_ply")]
                {
                    let cursor = Cursor::new(bytes);
                    let mut f = BufReader::new(cursor);

                    Ok(crate::io::ply::parse_ply_4d(&mut f)?)
                }

                #[cfg(not(feature = "io_ply"))]
                {
                    Err(std::io::Error::other(
                        "ply4d support not enabled, enable with io_ply feature",
                    ))
                }
            }
            Some("gc4d") => Ok(PlanarGaussian4d::decode(bytes.as_slice())),
            _ => Err(std::io::Error::other("only .ply4d and .gc4d supported")),
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ply4d", "gc4d"]
    }
}
