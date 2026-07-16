use static_assertions::assert_cfg;

// If both codecs are enabled, prefer flexbuffers.
#[cfg(all(feature = "io_bincode2", not(feature = "io_flexbuffers")))]
pub mod bincode2;

#[cfg(feature = "io_flexbuffers")]
pub mod flexbuffers;

assert_cfg!(
    any(feature = "io_bincode2", feature = "io_flexbuffers",),
    "no gcloud io enabled",
);
