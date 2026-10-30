use std::io::Write;

// TODO: support streamed codecs
pub trait CloudCodec {
    fn encode(&self) -> Vec<u8>;
    fn decode(data: &[u8]) -> Self;

    fn write_to_file(&self, path: &str) {
        let gcloud_file = std::fs::File::create(path).expect("failed to create file");
        let mut gcloud_writer = std::io::BufWriter::new(gcloud_file);

        let data = self.encode();
        gcloud_writer
            .write_all(data.as_slice())
            .expect("failed to write to gcloud file");
    }
}
