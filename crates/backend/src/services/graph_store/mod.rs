use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use shared::{
    GraphDocument, GraphExchangeFile, GraphImportCollisionPolicy, GraphImportMode, GraphMetadata,
    validate_graph_document,
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub(crate) trait GraphStoreEventPublisher: Send + Sync {
    /// Broadcasts the current graph metadata list after the store changes.
    fn graph_metadata_changed(&self, documents: Vec<GraphMetadata>);
}

pub(crate) struct GraphStore {
    dir: PathBuf,
    lock: Mutex<()>,
    events: Arc<dyn GraphStoreEventPublisher>,
}

#[derive(Debug)]
pub(crate) struct GraphImportResult {
    pub(crate) document: GraphDocument,
    pub(crate) mode: GraphImportMode,
}

impl GraphStore {
    /// Creates a graph store rooted at `root_dir`.
    ///
    /// Graph documents are stored beneath the `graph_documents` subdirectory, and metadata
    /// changes are reported through `events`.
    pub(crate) fn new(root_dir: &Path, events: Arc<dyn GraphStoreEventPublisher>) -> Self {
        Self {
            dir: root_dir.join("graph_documents"),
            lock: Mutex::new(()),
            events,
        }
    }

    /// Creates and persists a new empty graph document.
    ///
    /// The document receives a fresh UUID, the provided display name, and the default execution
    /// frequency. A metadata-changed event is emitted after the file is written.
    pub(crate) async fn create_graph_document(
        &self,
        name: String,
    ) -> anyhow::Result<GraphMetadata> {
        let metadata = GraphMetadata {
            id: Uuid::new_v4().to_string(),
            name,
            execution_frequency_hz: 60,
        };
        {
            let _guard = self.lock.lock().await;
            self.ensure_dir().await?;

            let document = GraphDocument {
                metadata: metadata.clone(),
                viewport: shared::GraphViewport::default(),
                nodes: Vec::new(),
                edges: Vec::new(),
            };

            let path = self.document_path(&metadata.id);
            let payload =
                serde_json::to_vec_pretty(&document).context("serialize graph document")?;
            tokio::fs::write(&path, payload)
                .await
                .with_context(|| format!("write graph document to {}", path.display()))?;
        }
        self.emit_metadata_changed().await?;

        Ok(metadata)
    }

    /// Deletes a graph document by ID.
    ///
    /// Returns `Ok(false)` when the document does not exist. When a file is removed, the store
    /// emits a metadata-changed event after the deletion completes.
    pub(crate) async fn delete_graph_document(&self, id: &str) -> anyhow::Result<bool> {
        let deleted = {
            let _guard = self.lock.lock().await;
            self.ensure_dir().await?;

            let path = self.document_path(id);
            match tokio::fs::remove_file(&path).await {
                Ok(()) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    return Err(error).with_context(|| format!("delete {}", path.display()));
                }
            }
        };

        if deleted {
            self.emit_metadata_changed().await?;
        }
        Ok(deleted)
    }

    /// Lists the metadata for every stored graph document.
    ///
    /// The returned list is sorted by graph name and then by graph ID.
    pub(crate) async fn list_graph_metadata(&self) -> anyhow::Result<Vec<GraphMetadata>> {
        let _guard = self.lock.lock().await;
        self.ensure_dir().await?;
        self.list_graph_metadata_unlocked().await
    }

    /// Loads a graph document by ID from persistent storage.
    ///
    /// Returns `Ok(None)` when the document does not exist.
    pub(crate) async fn get_graph_document(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<GraphDocument>> {
        let _guard = self.lock.lock().await;
        self.ensure_dir().await?;

        let path = self.document_path(id);
        let payload = match tokio::fs::read_to_string(&path).await {
            Ok(payload) => payload,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        let document = serde_json::from_str::<GraphDocument>(&payload)
            .with_context(|| format!("parse graph document {}", path.display()))?;
        Ok(Some(document))
    }

    /// Exports a graph document as a versioned exchange file.
    ///
    /// Returns `Ok(None)` when the graph does not exist.
    pub(crate) async fn export_graph_document(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<GraphExchangeFile>> {
        Ok(self
            .get_graph_document(id)
            .await?
            .map(GraphExchangeFile::new))
    }

    /// Saves a graph document to persistent storage.
    ///
    /// A metadata-changed event is emitted only when the persisted graph metadata differs from
    /// the previous version on disk.
    pub(crate) async fn save_graph_document(&self, document: &GraphDocument) -> anyhow::Result<()> {
        let id = document.metadata.id.trim();
        anyhow::ensure!(!id.is_empty(), "graph document id must not be empty");
        let metadata_changed = {
            let _guard = self.lock.lock().await;
            self.ensure_dir().await?;
            let path = self.document_path(id);
            let previous_metadata = match tokio::fs::read_to_string(&path).await {
                Ok(payload) => serde_json::from_str::<GraphDocument>(&payload)
                    .ok()
                    .map(|document| document.metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(error).with_context(|| format!("read {}", path.display()));
                }
            };
            let payload =
                serde_json::to_vec_pretty(document).context("serialize graph document")?;
            tokio::fs::write(&path, payload)
                .await
                .with_context(|| format!("write graph document to {}", path.display()))?;
            previous_metadata.as_ref() != Some(&document.metadata)
        };
        if metadata_changed {
            self.emit_metadata_changed().await?;
        }
        Ok(())
    }

    /// Updates the execution frequency stored in a graph document.
    ///
    /// Returns `Ok(false)` when the graph does not exist. The stored frequency is clamped to at
    /// least `1` Hz, and successful updates emit a metadata-changed event.
    pub(crate) async fn update_execution_frequency(
        &self,
        id: &str,
        execution_frequency_hz: u32,
    ) -> anyhow::Result<bool> {
        let found = {
            let _guard = self.lock.lock().await;
            self.ensure_dir().await?;
            let path = self.document_path(id);
            let payload = match tokio::fs::read_to_string(&path).await {
                Ok(payload) => payload,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => {
                    return Err(error).with_context(|| format!("read {}", path.display()));
                }
            };
            let mut document = serde_json::from_str::<GraphDocument>(&payload)
                .with_context(|| format!("parse graph document {}", path.display()))?;
            document.metadata.execution_frequency_hz = execution_frequency_hz.max(1);
            let payload =
                serde_json::to_vec_pretty(&document).context("serialize graph document")?;
            tokio::fs::write(&path, payload)
                .await
                .with_context(|| format!("write graph document to {}", path.display()))?;
            true
        };

        if found {
            self.emit_metadata_changed().await?;
        }
        Ok(found)
    }

    /// Updates the persisted display name of a graph document.
    ///
    /// Returns `Ok(false)` when the graph does not exist. Empty or whitespace-only names are
    /// rejected, and successful updates emit a metadata-changed event.
    pub(crate) async fn update_graph_name(&self, id: &str, name: String) -> anyhow::Result<bool> {
        let trimmed_name = name.trim().to_owned();
        anyhow::ensure!(
            !trimmed_name.is_empty(),
            "graph document name must not be empty"
        );

        let found = {
            let _guard = self.lock.lock().await;
            self.ensure_dir().await?;
            let path = self.document_path(id);
            let payload = match tokio::fs::read_to_string(&path).await {
                Ok(payload) => payload,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => {
                    return Err(error).with_context(|| format!("read {}", path.display()));
                }
            };
            let mut document = serde_json::from_str::<GraphDocument>(&payload)
                .with_context(|| format!("parse graph document {}", path.display()))?;
            document.metadata.name = trimmed_name;
            let payload =
                serde_json::to_vec_pretty(&document).context("serialize graph document")?;
            tokio::fs::write(&path, payload)
                .await
                .with_context(|| format!("write graph document to {}", path.display()))?;
            true
        };

        if found {
            self.emit_metadata_changed().await?;
        }
        Ok(found)
    }

    /// Imports a graph exchange file into persistent storage.
    ///
    /// The exchange header and graph contents are validated before anything is written. When the
    /// target graph ID already exists, `collision_policy` decides whether the import fails,
    /// overwrites the existing document, or creates a renamed copy with a fresh UUID.
    pub(crate) async fn import_graph_document(
        &self,
        file: GraphExchangeFile,
        collision_policy: GraphImportCollisionPolicy,
    ) -> anyhow::Result<GraphImportResult> {
        file.validate_header().map_err(anyhow::Error::msg)?;
        let mut document = file.document;
        let validation_issues = validate_graph_document(&document);
        anyhow::ensure!(
            validation_issues.is_empty(),
            "Imported graph document is invalid: {}",
            validation_issues
                .into_iter()
                .map(|issue| issue.message)
                .collect::<Vec<_>>()
                .join("; ")
        );
        let id = document.metadata.id.trim().to_owned();
        anyhow::ensure!(!id.is_empty(), "graph document id must not be empty");
        anyhow::ensure!(
            !document.metadata.name.trim().is_empty(),
            "graph document name must not be empty"
        );

        let _guard = self.lock.lock().await;
        self.ensure_dir().await?;

        let existing_path = self.document_path(&id);
        let exists = tokio::fs::try_exists(&existing_path)
            .await
            .with_context(|| format!("check {}", existing_path.display()))?;

        let mode = match (exists, collision_policy) {
            (false, _) | (_, GraphImportCollisionPolicy::PreserveIfFree) if !exists => {
                GraphImportMode::Imported
            }
            (false, _) => GraphImportMode::Imported,
            (true, GraphImportCollisionPolicy::OverwriteExisting) => GraphImportMode::Overwritten,
            (true, GraphImportCollisionPolicy::ImportCopy) => {
                let existing_metadata = self.list_graph_metadata_unlocked().await?;
                document.metadata.id = Uuid::new_v4().to_string();
                document.metadata.name =
                    next_imported_graph_name(&document.metadata.name, &existing_metadata);
                GraphImportMode::Imported
            }
            (true, GraphImportCollisionPolicy::PreserveIfFree) => {
                anyhow::bail!("Graph document {id} already exists")
            }
        };

        let path = self.document_path(&document.metadata.id);
        let payload = serde_json::to_vec_pretty(&document).context("serialize graph document")?;
        tokio::fs::write(&path, payload)
            .await
            .with_context(|| format!("write graph document to {}", path.display()))?;
        drop(_guard);

        self.emit_metadata_changed().await?;

        Ok(GraphImportResult { document, mode })
    }

    /// Emits a metadata-changed event with the current graph metadata list.
    async fn emit_metadata_changed(&self) -> anyhow::Result<()> {
        let documents = self.list_graph_metadata().await?;
        self.events.graph_metadata_changed(documents);
        Ok(())
    }

    /// Ensures that the graph storage directory exists.
    async fn ensure_dir(&self) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .with_context(|| format!("create {}", self.dir.display()))
    }

    /// Lists graph metadata without taking the outer store lock.
    ///
    /// Callers must already hold the store lock before using this helper.
    async fn list_graph_metadata_unlocked(&self) -> anyhow::Result<Vec<GraphMetadata>> {
        let mut entries = tokio::fs::read_dir(&self.dir)
            .await
            .with_context(|| format!("read {}", self.dir.display()))?;
        let mut metadata = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| format!("iterate {}", self.dir.display()))?
        {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let payload = tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("read graph document {}", path.display()))?;
            let document = serde_json::from_str::<GraphDocument>(&payload)
                .with_context(|| format!("parse graph document {}", path.display()))?;
            metadata.push(document.metadata);
        }

        metadata.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(metadata)
    }

    /// Returns the filesystem path for a graph document ID.
    fn document_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }
}

/// Returns the next available imported graph name.
///
/// The first imported copy uses the `"(Imported)"` suffix, and subsequent collisions add an
/// incrementing numeric suffix.
fn next_imported_graph_name(name: &str, existing_graphs: &[GraphMetadata]) -> String {
    let base_name = format!("{} (Imported)", name.trim());
    let existing_names = existing_graphs
        .iter()
        .map(|graph| graph.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    if !existing_names.contains(base_name.as_str()) {
        return base_name;
    }

    let mut suffix = 2usize;
    loop {
        let candidate = format!("{base_name} {suffix}");
        if !existing_names.contains(candidate.as_str()) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use shared::{
        GraphDocument, GraphEdge, GraphExchangeFile, GraphImportCollisionPolicy, GraphMetadata,
        GraphNode, NodeMetadata, NodeTypeId,
    };

    use super::{GraphStore, GraphStoreEventPublisher};

    struct NoopEvents;

    impl GraphStoreEventPublisher for NoopEvents {
        /// Ignores metadata-change broadcasts in graph-store tests.
        fn graph_metadata_changed(&self, _documents: Vec<GraphMetadata>) {}
    }

    /// Creates an isolated graph store rooted in a temporary directory.
    fn test_store() -> (GraphStore, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("graph_store_test_{}", uuid::Uuid::new_v4()));
        (GraphStore::new(&root, Arc::new(NoopEvents)), root)
    }

    /// Builds a minimal graph document for storage tests.
    fn minimal_document(id: &str, name: &str) -> GraphDocument {
        GraphDocument {
            metadata: GraphMetadata {
                id: id.to_owned(),
                name: name.to_owned(),
                execution_frequency_hz: 60,
            },
            viewport: shared::GraphViewport::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    #[tokio::test]
    /// Tests that export wraps a stored graph document in the exchange-file format.
    async fn export_returns_wrapped_graph_document() {
        let (store, root) = test_store();
        let document = minimal_document("graph-1", "Export Me");
        store.save_graph_document(&document).await.unwrap();

        let exported = store
            .export_graph_document("graph-1")
            .await
            .unwrap()
            .expect("exported document");

        assert_eq!(exported, GraphExchangeFile::new(document));
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    /// Tests that importing a graph with a free ID preserves its ID and metadata.
    async fn import_preserves_free_graph_id() {
        let (store, root) = test_store();
        let document = minimal_document("graph-1", "Imported Graph");

        let result = store
            .import_graph_document(
                GraphExchangeFile::new(document.clone()),
                GraphImportCollisionPolicy::PreserveIfFree,
            )
            .await
            .unwrap();

        assert_eq!(result.document, document);
        assert_eq!(result.mode, shared::GraphImportMode::Imported);
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    /// Tests that import-copy assigns a new ID and imported name when the graph already exists.
    async fn import_copy_reassigns_id_and_name_when_graph_exists() {
        let (store, root) = test_store();
        store
            .save_graph_document(&minimal_document("graph-1", "Imported Graph"))
            .await
            .unwrap();

        let result = store
            .import_graph_document(
                GraphExchangeFile::new(minimal_document("graph-1", "Imported Graph")),
                GraphImportCollisionPolicy::ImportCopy,
            )
            .await
            .unwrap();

        assert_ne!(result.document.metadata.id, "graph-1");
        assert_eq!(result.document.metadata.name, "Imported Graph (Imported)");
        assert_eq!(result.mode, shared::GraphImportMode::Imported);
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    /// Tests that overwrite import replaces the existing stored graph document.
    async fn overwrite_replaces_existing_graph_document() {
        let (store, root) = test_store();
        store
            .save_graph_document(&minimal_document("graph-1", "Original"))
            .await
            .unwrap();

        let replacement = minimal_document("graph-1", "Replacement");
        let result = store
            .import_graph_document(
                GraphExchangeFile::new(replacement.clone()),
                GraphImportCollisionPolicy::OverwriteExisting,
            )
            .await
            .unwrap();

        let saved = store
            .get_graph_document("graph-1")
            .await
            .unwrap()
            .expect("saved graph");
        assert_eq!(saved, replacement);
        assert_eq!(result.mode, shared::GraphImportMode::Overwritten);
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    /// Tests that invalid imports are rejected before any graph file is written.
    async fn invalid_import_is_rejected_without_writing() {
        let (store, root) = test_store();
        let document = GraphDocument {
            metadata: GraphMetadata {
                id: "graph-1".to_owned(),
                name: "Broken".to_owned(),
                execution_frequency_hz: 60,
            },
            viewport: shared::GraphViewport::default(),
            nodes: vec![GraphNode {
                id: "node-1".to_owned(),
                metadata: NodeMetadata {
                    name: "Broken Node".to_owned(),
                },
                node_type: NodeTypeId::new("unknown.node"),
                viewport: shared::NodeViewport::default(),
                input_values: Vec::new(),
                parameters: Vec::new(),
            }],
            edges: vec![GraphEdge {
                from_node_id: "node-1".to_owned(),
                from_output_name: "value".to_owned(),
                to_node_id: "node-2".to_owned(),
                to_input_name: "value".to_owned(),
            }],
        };

        let error = store
            .import_graph_document(
                GraphExchangeFile::new(document),
                GraphImportCollisionPolicy::PreserveIfFree,
            )
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Imported graph document is invalid")
        );
        assert!(
            store.get_graph_document("graph-1").await.unwrap().is_none(),
            "invalid graph should not be written"
        );
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
