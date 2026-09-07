#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub struct Fixture(pub PathBuf);

impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "perro_export_probe_{}_{}_{}",
            std::process::id(),
            stamp,
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("res")).expect("create fixture");
        Self(root)
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.0.join("res").join(rel);
        fs::create_dir_all(path.parent().expect("source parent")).expect("create source dir");
        fs::write(path, bytes).expect("write source");
    }

    pub fn tree(&self) -> perro_static_pipeline::ResFileTree {
        perro_static_pipeline::ResFileTree::scan(&self.0).expect("scan fixture")
    }

    pub fn clear_outputs(&self) {
        let output = self.0.join(".perro");
        if output.exists() {
            fs::remove_dir_all(output).expect("clear fixture outputs");
        }
    }

    pub fn image(&self, rel: &str, side: u32) {
        let image = image::RgbaImage::from_fn(side, side, |x, y| {
            let mut n = x
                .wrapping_add(y.wrapping_mul(side))
                .wrapping_add(0x9e37_79b9);
            n ^= n >> 16;
            n = n.wrapping_mul(0x85eb_ca6b);
            n ^= n >> 13;
            n = n.wrapping_mul(0xc2b2_ae35);
            n ^= n >> 16;
            image::Rgba([n as u8, (n >> 8) as u8, (n >> 16) as u8, 255])
        });
        image.save(self.0.join("res").join(rel)).expect("write PNG");
    }

    pub fn model(&self, name: &str, image: &str) {
        let mut buffer = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0u16, 1, 2] {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        self.write("vertices.bin", &buffer);
        let document = format!(
            r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"uri":"vertices.bin","byteLength":42}}],"bufferViews":[{{"buffer":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],"images":[{{"uri":"{image}"}},{{"uri":"{image}"}},{{"uri":"{image}"}},{{"uri":"{image}"}}],"textures":[{{"source":0}}],"materials":[{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":0}},"roughnessFactor":0.75}}}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0}}]}}],"nodes":[{{"mesh":0}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#
        );
        self.write(name, document.as_bytes());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn outputs(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for item in fs::read_dir(path).expect("list output") {
            let path = item.expect("output entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if !path
                .file_name()
                .expect("output name")
                .to_string_lossy()
                .starts_with('.')
            {
                out.insert(
                    path.strip_prefix(root)
                        .expect("relative output")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    fs::read(path).expect("read output"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
