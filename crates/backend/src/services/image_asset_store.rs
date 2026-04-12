use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use uuid::Uuid;

use crate::services::image_codec::{is_svg_payload, parse_svg};

static GLOBAL_IMAGE_ASSET_STORE: OnceLock<Arc<ImageAssetStore>> = OnceLock::new();

/// Registers the global image asset store used by runtime nodes.
pub(crate) fn set_global_image_asset_store(store: Arc<ImageAssetStore>) -> anyhow::Result<()> {
    GLOBAL_IMAGE_ASSET_STORE
        .set(store)
        .map_err(|_| anyhow::anyhow!("global image asset store already initialized"))
}

/// Returns the process-wide image asset store when backend startup initialized it.
pub(crate) fn global_image_asset_store() -> Option<&'static Arc<ImageAssetStore>> {
    GLOBAL_IMAGE_ASSET_STORE.get()
}

/// Persists uploaded image bytes so graph nodes can reference them by stable asset id.
pub(crate) struct ImageAssetStore {
    asset_dir: PathBuf,
}

impl ImageAssetStore {
    /// Creates the asset store rooted under the backend data directory.
    pub(crate) fn new(data_dir: &Path) -> anyhow::Result<Self> {
        let asset_dir = data_dir.join("assets").join("images");
        fs::create_dir_all(&asset_dir)
            .with_context(|| format!("create image asset directory {}", asset_dir.display()))?;
        Ok(Self { asset_dir })
    }

    /// Validates and stores uploaded image bytes, returning the new asset id.
    pub(crate) fn store_image_bytes(&self, bytes: &[u8]) -> anyhow::Result<String> {
        anyhow::ensure!(!bytes.is_empty(), "uploaded image is empty");
        validate_uploaded_image(bytes)?;

        let asset_id = Uuid::new_v4().to_string();
        let path = self.asset_path(&asset_id)?;
        fs::write(&path, bytes)
            .with_context(|| format!("write uploaded image asset {}", path.display()))?;
        Ok(asset_id)
    }

    /// Loads the original bytes for a previously uploaded image asset.
    pub(crate) fn load_image_bytes(&self, asset_id: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.asset_path(asset_id)?;
        fs::read(&path).with_context(|| format!("read image asset {}", path.display()))
    }

    fn asset_path(&self, asset_id: &str) -> anyhow::Result<PathBuf> {
        let parsed = Uuid::parse_str(asset_id)
            .with_context(|| format!("image asset id '{asset_id}' is invalid"))?;
        Ok(self.asset_dir.join(parsed.to_string()))
    }
}

fn validate_uploaded_image(bytes: &[u8]) -> anyhow::Result<()> {
    if is_svg_payload(bytes) {
        parse_svg(bytes, "decode uploaded image")?;
        return Ok(());
    }

    image::load_from_memory(bytes).context("decode uploaded image")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use image::{DynamicImage, ImageFormat, RgbaImage};

    use super::ImageAssetStore;

    fn temp_test_dir(test_name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("luma-weaver-{test_name}-{}", uuid::Uuid::new_v4()))
    }

    fn sample_png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_vec(
                2,
                1,
                vec![
                    255, 0, 0, 255, //
                    0, 0, 255, 255,
                ],
            )
            .expect("construct test image"),
        );
        image
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode png");
        bytes
    }

    #[test]
    fn stores_and_loads_uploaded_image_bytes() {
        let data_dir = temp_test_dir("stores-and-loads-uploaded-image-bytes");
        let store = ImageAssetStore::new(&data_dir).expect("create image asset store");
        let bytes = sample_png_bytes();

        let asset_id = store.store_image_bytes(&bytes).expect("store image asset");
        let loaded = store
            .load_image_bytes(&asset_id)
            .expect("load stored image asset");

        assert_eq!(loaded, bytes);

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn rejects_invalid_uploaded_image_bytes() {
        let data_dir = temp_test_dir("rejects-invalid-uploaded-image-bytes");
        let store = ImageAssetStore::new(&data_dir).expect("create image asset store");

        let error = store
            .store_image_bytes(b"not an image")
            .expect_err("invalid image bytes should fail");

        assert!(error.to_string().contains("decode uploaded image"));

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn accepts_uploaded_svg_bytes() {
        let data_dir = temp_test_dir("accepts-uploaded-svg-bytes");
        let store = ImageAssetStore::new(&data_dir).expect("create image asset store");

        let asset_id = store
            .store_image_bytes(
                br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#ff0000"/></svg>"##,
            )
            .expect("store svg image asset");

        let loaded = store
            .load_image_bytes(&asset_id)
            .expect("load stored image asset");
        assert!(!loaded.is_empty());

        let _ = fs::remove_dir_all(data_dir);
    }
}
