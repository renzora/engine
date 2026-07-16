use flexbuffers::{FlexbufferSerializer, Reader};
use serde::{Deserialize, Serialize};

use crate::{
    gaussian::formats::{planar_3d::PlanarGaussian3d, planar_4d::PlanarGaussian4d},
    io::codec::CloudCodec,
};

impl CloudCodec for PlanarGaussian3d {
    fn encode(&self) -> Vec<u8> {
        let mut serializer = FlexbufferSerializer::new();
        self.serialize(&mut serializer)
            .expect("failed to serialize cloud");

        serializer.view().to_vec()
    }

    fn decode(data: &[u8]) -> Self {
        let reader = Reader::get_root(data).expect("failed to read flexbuffer");
        Self::deserialize(reader).expect("deserialization failed")
    }
}

impl CloudCodec for PlanarGaussian4d {
    fn encode(&self) -> Vec<u8> {
        let mut serializer = FlexbufferSerializer::new();
        self.serialize(&mut serializer)
            .expect("failed to serialize cloud");

        serializer.view().to_vec()
    }

    fn decode(data: &[u8]) -> Self {
        let reader = Reader::get_root(data).expect("failed to read flexbuffer");
        Self::deserialize(reader).expect("deserialization failed")
    }
}
