use super::GraphDocument;
use serde::{Deserialize, Serialize};

/// Stable file-format identifier for exported graph documents.
pub const GRAPH_EXCHANGE_FORMAT: &str = "animation_builder_graph";
/// Current version of the single-graph exchange file format.
pub const GRAPH_EXCHANGE_VERSION: u32 = 1;

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
    use super::{GRAPH_EXCHANGE_FORMAT, GRAPH_EXCHANGE_VERSION, GraphDocument, GraphExchangeFile};

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
}
