use super::{GraphDocument, GraphEdge, GraphNode, NodePosition};
use serde::{Deserialize, Serialize};

/// Stable file-format identifier for exported graph documents.
pub const GRAPH_EXCHANGE_FORMAT: &str = "animation_builder_graph";
/// Current version of the single-graph exchange file format.
pub const GRAPH_EXCHANGE_VERSION: u32 = 1;
/// Stable clipboard-format identifier for copied graph fragments.
pub const GRAPH_CLIPBOARD_FRAGMENT_FORMAT: &str = "animation_builder_graph_fragment";
/// Current version of the clipboard fragment format.
pub const GRAPH_CLIPBOARD_FRAGMENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Selects how an imported graph should behave when its ID already exists locally.
pub enum GraphImportCollisionPolicy {
    PreserveIfFree,
    ImportCopy,
    OverwriteExisting,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Reports how an import operation affected local graph storage.
pub enum GraphImportMode {
    Imported,
    Overwritten,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents the versioned clipboard format for a copied graph fragment.
pub struct GraphClipboardFragment {
    pub format: String,
    pub version: u32,
    pub origin: NodePosition,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
}

impl GraphClipboardFragment {
    /// Wraps a copied node fragment in the current clipboard header.
    pub fn new(origin: NodePosition, nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        Self {
            format: GRAPH_CLIPBOARD_FRAGMENT_FORMAT.to_owned(),
            version: GRAPH_CLIPBOARD_FRAGMENT_VERSION,
            origin,
            nodes,
            edges,
        }
    }

    /// Validates that the clipboard header matches the supported format and version.
    pub fn validate_header(&self) -> Result<(), String> {
        if self.format != GRAPH_CLIPBOARD_FRAGMENT_FORMAT {
            return Err(format!(
                "Unsupported graph clipboard format '{}'",
                self.format
            ));
        }
        if self.version != GRAPH_CLIPBOARD_FRAGMENT_VERSION {
            return Err(format!(
                "Unsupported graph clipboard version {}",
                self.version
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents the versioned on-disk exchange format for a single exported graph.
pub struct GraphExchangeFile {
    pub format: String,
    pub version: u32,
    pub document: GraphDocument,
}

impl GraphExchangeFile {
    /// Wraps a graph document in the current exchange file header.
    pub fn new(document: GraphDocument) -> Self {
        Self {
            format: GRAPH_EXCHANGE_FORMAT.to_owned(),
            version: GRAPH_EXCHANGE_VERSION,
            document,
        }
    }

    /// Validates that the exchange header matches the supported format and version.
    pub fn validate_header(&self) -> Result<(), String> {
        if self.format != GRAPH_EXCHANGE_FORMAT {
            return Err(format!(
                "Unsupported graph exchange format '{}'",
                self.format
            ));
        }
        if self.version != GRAPH_EXCHANGE_VERSION {
            return Err(format!(
                "Unsupported graph exchange version {}",
                self.version
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod graph_exchange_tests {
    use super::{
        GRAPH_CLIPBOARD_FRAGMENT_FORMAT, GRAPH_CLIPBOARD_FRAGMENT_VERSION, GRAPH_EXCHANGE_FORMAT,
        GRAPH_EXCHANGE_VERSION, GraphClipboardFragment, GraphDocument, GraphExchangeFile,
    };
    use crate::NodePosition;

    /// Tests that graph exchange files preserve their contents across JSON serialization.
    #[test]
    fn graph_exchange_file_round_trips_through_json() {
        let file = GraphExchangeFile::new(GraphDocument::default());
        let json = serde_json::to_string(&file).expect("serialize graph exchange file");
        let decoded: GraphExchangeFile =
            serde_json::from_str(&json).expect("deserialize graph exchange file");
        assert_eq!(decoded, file);
    }

    /// Tests that an unexpected exchange format identifier is rejected.
    #[test]
    fn graph_exchange_file_rejects_unknown_format() {
        let file = GraphExchangeFile {
            format: "other_format".to_owned(),
            version: GRAPH_EXCHANGE_VERSION,
            document: GraphDocument::default(),
        };

        assert_eq!(
            file.validate_header().unwrap_err(),
            "Unsupported graph exchange format 'other_format'"
        );
    }

    /// Tests that an unsupported exchange version is rejected.
    #[test]
    fn graph_exchange_file_rejects_unknown_version() {
        let file = GraphExchangeFile {
            format: GRAPH_EXCHANGE_FORMAT.to_owned(),
            version: GRAPH_EXCHANGE_VERSION + 1,
            document: GraphDocument::default(),
        };

        assert_eq!(
            file.validate_header().unwrap_err(),
            format!(
                "Unsupported graph exchange version {}",
                GRAPH_EXCHANGE_VERSION + 1
            )
        );
    }

    #[test]
    fn graph_clipboard_fragment_round_trips_through_json() {
        let fragment =
            GraphClipboardFragment::new(NodePosition { x: 12.0, y: 34.0 }, Vec::new(), Vec::new());
        let json = serde_json::to_string(&fragment).expect("serialize graph clipboard fragment");
        let decoded: GraphClipboardFragment =
            serde_json::from_str(&json).expect("deserialize graph clipboard fragment");
        assert_eq!(decoded, fragment);
    }

    #[test]
    fn graph_clipboard_fragment_rejects_unknown_format() {
        let fragment = GraphClipboardFragment {
            format: "other_format".to_owned(),
            version: GRAPH_CLIPBOARD_FRAGMENT_VERSION,
            origin: NodePosition::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        assert_eq!(
            fragment.validate_header().unwrap_err(),
            "Unsupported graph clipboard format 'other_format'"
        );
    }

    #[test]
    fn graph_clipboard_fragment_rejects_unknown_version() {
        let fragment = GraphClipboardFragment {
            format: GRAPH_CLIPBOARD_FRAGMENT_FORMAT.to_owned(),
            version: GRAPH_CLIPBOARD_FRAGMENT_VERSION + 1,
            origin: NodePosition::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        assert_eq!(
            fragment.validate_header().unwrap_err(),
            format!(
                "Unsupported graph clipboard version {}",
                GRAPH_CLIPBOARD_FRAGMENT_VERSION + 1
            )
        );
    }
}
