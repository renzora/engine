use bincode2::{deserialize_from, serialize_into};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::de::DeserializeOwned;
use std::io::Cursor;

use crate::{
    gaussian::formats::{planar_3d::PlanarGaussian3d, planar_4d::PlanarGaussian4d},
    io::codec::CloudCodec,
};

impl CloudCodec for PlanarGaussian3d {
    fn encode(&self) -> Vec<u8> {
        let mut output = Vec::new();

        {
            let mut gz_encoder = GzEncoder::new(&mut output, Compression::default());
            serialize_into(&mut gz_encoder, &self).expect("failed to encode cloud");
        }

        output
    }

    fn decode(data: &[u8]) -> Self {
        if let Ok(cloud) = decode_gzip(data) {
            return cloud;
        }

        decode_raw(data)
    }
}

impl CloudCodec for PlanarGaussian4d {
    fn encode(&self) -> Vec<u8> {
        let mut output = Vec::new();

        {
            let mut gz_encoder = GzEncoder::new(&mut output, Compression::default());
            serialize_into(&mut gz_encoder, &self).expect("failed to encode cloud");
        }

        output
    }

    fn decode(data: &[u8]) -> Self {
        if let Ok(cloud) = decode_gzip(data) {
            return cloud;
        }

        decode_raw(data)
    }
}

fn decode_gzip<T>(data: &[u8]) -> Result<T, bincode2::Error>
where
    T: DeserializeOwned,
{
    let decompressed = GzDecoder::new(data);
    deserialize_from(decompressed)
}

fn decode_raw<T>(data: &[u8]) -> T
where
    T: DeserializeOwned,
{
    deserialize_from(Cursor::new(data)).expect("failed to decode cloud")
}
