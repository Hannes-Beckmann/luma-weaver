use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use shared::Vec3;
use uuid::Uuid;

static GLOBAL_LAYOUT_ASSET_STORE: OnceLock<Arc<LayoutAssetStore>> = OnceLock::new();

/// Registers the global layout asset store used by spatial layout helpers.
pub(crate) fn set_global_layout_asset_store(store: Arc<LayoutAssetStore>) -> anyhow::Result<()> {
    GLOBAL_LAYOUT_ASSET_STORE
        .set(store)
        .map_err(|_| anyhow::anyhow!("global layout asset store already initialized"))
}

/// Returns the process-wide layout asset store when backend startup initialized it.
pub(crate) fn global_layout_asset_store() -> Option<&'static Arc<LayoutAssetStore>> {
    GLOBAL_LAYOUT_ASSET_STORE.get()
}

/// Persists uploaded layout asset bytes so graph nodes can reference them by stable asset id.
pub(crate) struct LayoutAssetStore {
    asset_dir: PathBuf,
}

impl LayoutAssetStore {
    /// Creates the asset store rooted under the backend data directory.
    pub(crate) fn new(data_dir: &Path) -> anyhow::Result<Self> {
        let asset_dir = data_dir.join("assets").join("layouts");
        fs::create_dir_all(&asset_dir)
            .with_context(|| format!("create layout asset directory {}", asset_dir.display()))?;
        Ok(Self { asset_dir })
    }

    /// Validates and stores uploaded layout bytes, returning the new asset id.
    pub(crate) fn store_layout_bytes(&self, bytes: &[u8]) -> anyhow::Result<String> {
        anyhow::ensure!(!bytes.is_empty(), "uploaded layout is empty");
        parse_layout_points(bytes)?;

        let asset_id = Uuid::new_v4().to_string();
        let path = self.asset_path(&asset_id)?;
        fs::write(&path, bytes)
            .with_context(|| format!("write uploaded layout asset {}", path.display()))?;
        Ok(asset_id)
    }

    /// Loads the original bytes for a previously uploaded layout asset.
    pub(crate) fn load_layout_bytes(&self, asset_id: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.asset_path(asset_id)?;
        fs::read(&path).with_context(|| format!("read layout asset {}", path.display()))
    }

    /// Loads and parses a previously uploaded layout asset into point coordinates.
    pub(crate) fn load_layout_points(&self, asset_id: &str) -> anyhow::Result<Vec<Vec3>> {
        let bytes = self.load_layout_bytes(asset_id)?;
        parse_layout_points(&bytes)
    }

    /// Deletes a previously uploaded layout asset when it is no longer referenced by any graph.
    pub(crate) fn delete_layout_asset(&self, asset_id: &str) -> anyhow::Result<()> {
        let path = self.asset_path(asset_id)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("delete layout asset {}", path.display())),
        }
    }

    fn asset_path(&self, asset_id: &str) -> anyhow::Result<PathBuf> {
        let parsed = Uuid::parse_str(asset_id)
            .with_context(|| format!("layout asset id '{asset_id}' is invalid"))?;
        Ok(self.asset_dir.join(parsed.to_string()))
    }
}

/// Parses uploaded layout bytes into a list of 3D points.
pub(crate) fn parse_layout_points(bytes: &[u8]) -> anyhow::Result<Vec<Vec3>> {
    let trimmed = std::str::from_utf8(bytes)
        .context("decode uploaded layout as UTF-8")?
        .trim();
    anyhow::ensure!(!trimmed.is_empty(), "uploaded layout is empty");

    if trimmed.starts_with('[') {
        let points: Vec<Vec3> =
            serde_json::from_str(trimmed).context("parse uploaded layout JSON point array")?;
        anyhow::ensure!(!points.is_empty(), "uploaded layout contains no points");
        return Ok(points);
    }

    parse_layout_csv(trimmed)
}

fn parse_layout_csv(csv_text: &str) -> anyhow::Result<Vec<Vec3>> {
    let mut lines = csv_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let header = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("uploaded layout CSV is empty"))?;
    let columns = header
        .split(',')
        .map(|column| column.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        columns == ["x", "y", "z"],
        "uploaded layout CSV must use exactly the headers x,y,z"
    );

    let mut points = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let values = line.split(',').map(str::trim).collect::<Vec<_>>();
        anyhow::ensure!(
            values.len() == 3,
            "uploaded layout CSV row {} must contain exactly 3 columns",
            line_index + 2
        );
        let parse = |value: &str, axis: &str| {
            value
                .parse::<f32>()
                .with_context(|| format!("parse uploaded layout CSV {axis} value '{value}'"))
        };
        points.push(Vec3 {
            x: parse(values[0], "x")?,
            y: parse(values[1], "y")?,
            z: parse(values[2], "z")?,
        });
    }

    anyhow::ensure!(!points.is_empty(), "uploaded layout contains no points");
    Ok(points)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use shared::Vec3;

    use super::{LayoutAssetStore, parse_layout_points};

    fn temp_test_dir(test_name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("luma-weaver-{test_name}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn parses_json_point_array() {
        let points =
            parse_layout_points(br#"[{"x":0.0,"y":1.0,"z":2.0},{"x":3.0,"y":4.0,"z":5.0}]"#)
                .expect("parse json point array");

        assert_eq!(
            points,
            vec![
                Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 2.0,
                },
                Vec3 {
                    x: 3.0,
                    y: 4.0,
                    z: 5.0,
                },
            ]
        );
    }

    #[test]
    fn parses_csv_points() {
        let points = parse_layout_points(b"x,y,z\n0,1,2\n3,4,5\n").expect("parse csv points");

        assert_eq!(
            points,
            vec![
                Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 2.0,
                },
                Vec3 {
                    x: 3.0,
                    y: 4.0,
                    z: 5.0,
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_csv_headers() {
        let error = parse_layout_points(b"x,y\n0,1\n").expect_err("invalid csv headers");

        assert!(error.to_string().contains("headers x,y,z"));
    }

    #[test]
    fn stores_and_loads_uploaded_layout_bytes() {
        let data_dir = temp_test_dir("stores-and-loads-uploaded-layout-bytes");
        let store = LayoutAssetStore::new(&data_dir).expect("create layout asset store");
        let bytes = b"x,y,z\n0,1,2\n".to_vec();

        let asset_id = store
            .store_layout_bytes(&bytes)
            .expect("store layout asset");
        let loaded = store
            .load_layout_bytes(&asset_id)
            .expect("load stored layout asset");

        assert_eq!(loaded, bytes);

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn deletes_uploaded_layout_asset() {
        let data_dir = temp_test_dir("deletes-uploaded-layout-asset");
        let store = LayoutAssetStore::new(&data_dir).expect("create layout asset store");
        let asset_id = store
            .store_layout_bytes(b"x,y,z\n0,1,2\n")
            .expect("store layout asset");

        store
            .delete_layout_asset(&asset_id)
            .expect("delete layout asset");

        let error = store
            .load_layout_bytes(&asset_id)
            .expect_err("deleted layout asset should not load");
        assert!(error.to_string().contains("read layout asset"));

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn deleting_missing_layout_asset_is_ok() {
        let data_dir = temp_test_dir("deleting-missing-layout-asset-is-ok");
        let store = LayoutAssetStore::new(&data_dir).expect("create layout asset store");
        let asset_id = uuid::Uuid::new_v4().to_string();

        store
            .delete_layout_asset(&asset_id)
            .expect("missing asset delete should be idempotent");

        let _ = fs::remove_dir_all(data_dir);
    }
}
