use shared::{ClientMessage, ServerMessage};

use super::RoutingContext;

/// Handles graph-document and graph-schema messages for a single client request.
///
/// This covers graph CRUD, import/export, rename, execution-frequency metadata updates, and
/// node-definition/metadata fetches. Validation errors are returned as user-facing
/// `ServerMessage::Error` responses rather than bubbling out as transport failures.
pub(super) async fn handle(
    context: &mut RoutingContext<'_>,
    message: ClientMessage,
) -> Option<ServerMessage> {
    match message {
        ClientMessage::CreateGraphDocument { name } if name.trim().is_empty() => {
            tracing::warn!(
                client_id = context.client_id,
                "received empty graph document name"
            );
            Some(ServerMessage::Error {
                message: "Graph document name must not be empty".to_owned(),
            })
        }
        ClientMessage::CreateGraphDocument { name } => {
            let trimmed_name = name.trim().to_owned();
            match context
                .state
                .graph_store
                .create_graph_document(trimmed_name)
                .await
            {
                Ok(_) => None,
                Err(error) => {
                    tracing::error!(client_id = context.client_id, %error, "failed to create graph document");
                    Some(ServerMessage::Error {
                        message: format!("Failed to create graph document: {error}"),
                    })
                }
            }
        }
        ClientMessage::DeleteGraphDocument { id } if id.trim().is_empty() => {
            tracing::warn!(
                client_id = context.client_id,
                "received empty graph document id"
            );
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::DeleteGraphDocument { id } => {
            let id = id.trim().to_owned();
            match context.state.graph_store.delete_graph_document(&id).await {
                Ok(true) => {
                    let running_update = context.state.runtime_manager.remove_graph(&id).await;
                    if let Err(error) = running_update {
                        tracing::error!(client_id = context.client_id, %error, %id, "failed to update runtime state");
                    }
                    None
                }
                Ok(false) => Some(ServerMessage::Error {
                    message: format!("Graph document {id} does not exist"),
                }),
                Err(error) => {
                    tracing::error!(client_id = context.client_id, %error, %id, "failed to delete graph document");
                    Some(ServerMessage::Error {
                        message: format!("Failed to delete graph document: {error}"),
                    })
                }
            }
        }
        ClientMessage::GetGraphDocument { id } if id.trim().is_empty() => {
            tracing::warn!(
                client_id = context.client_id,
                "received empty graph document id"
            );
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::GetGraphDocument { id } => {
            let id = id.trim().to_owned();
            match context.state.graph_store.get_graph_document(&id).await {
                Ok(Some(document)) => Some(ServerMessage::GraphDocument { document }),
                Ok(None) => Some(ServerMessage::Error {
                    message: format!("Graph document {id} does not exist"),
                }),
                Err(error) => {
                    tracing::error!(client_id = context.client_id, %error, %id, "failed to load graph document");
                    Some(ServerMessage::Error {
                        message: format!("Failed to load graph document: {error}"),
                    })
                }
            }
        }
        ClientMessage::ExportGraphDocument { id } if id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::ExportGraphDocument { id } => {
            let id = id.trim().to_owned();
            match context.state.graph_store.export_graph_document(&id).await {
                Ok(Some(file)) => Some(ServerMessage::GraphExport { file }),
                Ok(None) => Some(ServerMessage::Error {
                    message: format!("Graph document {id} does not exist"),
                }),
                Err(error) => {
                    tracing::error!(client_id = context.client_id, %error, %id, "failed to export graph document");
                    Some(ServerMessage::Error {
                        message: format!("Failed to export graph document: {error}"),
                    })
                }
            }
        }
        ClientMessage::UpdateGraphDocument { document } => {
            let id = document.metadata.id.trim().to_owned();
            if id.is_empty() {
                Some(ServerMessage::Error {
                    message: "Graph document id must not be empty".to_owned(),
                })
            } else {
                match context
                    .state
                    .graph_store
                    .save_graph_document(&document)
                    .await
                {
                    Ok(()) => Some(ServerMessage::GraphDocument { document }),
                    Err(error) => {
                        tracing::error!(client_id = context.client_id, %error, %id, "failed to save graph document");
                        Some(ServerMessage::Error {
                            message: format!("Failed to save graph document: {error}"),
                        })
                    }
                }
            }
        }
        ClientMessage::UpdateGraphName { id, name: _ } if id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::UpdateGraphName { id: _, name } if name.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document name must not be empty".to_owned(),
            })
        }
        ClientMessage::UpdateGraphName { id, name } => {
            let id = id.trim().to_owned();
            let name = name.trim().to_owned();
            match context.state.graph_store.update_graph_name(&id, name).await {
                Ok(true) => None,
                Ok(false) => Some(ServerMessage::Error {
                    message: format!("Graph document {id} does not exist"),
                }),
                Err(error) => {
                    tracing::error!(client_id = context.client_id, %error, %id, "failed to update graph name");
                    Some(ServerMessage::Error {
                        message: format!("Failed to update graph name: {error}"),
                    })
                }
            }
        }
        ClientMessage::ImportGraphDocument {
            file,
            collision_policy,
        } => {
            if let Err(message) = file.validate_header() {
                Some(ServerMessage::Error { message })
            } else {
                match context
                    .state
                    .graph_store
                    .import_graph_document(file, collision_policy)
                    .await
                {
                    Ok(result) => Some(ServerMessage::GraphImported {
                        document: result.document,
                        mode: result.mode,
                    }),
                    Err(error) => {
                        tracing::error!(client_id = context.client_id, %error, "failed to import graph document");
                        Some(ServerMessage::Error {
                            message: format!("Failed to import graph document: {error}"),
                        })
                    }
                }
            }
        }
        ClientMessage::UpdateGraphExecutionFrequency {
            id,
            execution_frequency_hz: _,
        } if id.trim().is_empty() => Some(ServerMessage::Error {
            message: "Graph document id must not be empty".to_owned(),
        }),
        ClientMessage::UpdateGraphExecutionFrequency {
            id,
            execution_frequency_hz,
        } => {
            let id = id.trim().to_owned();
            if execution_frequency_hz == 0 {
                Some(ServerMessage::Error {
                    message: "Execution frequency must be greater than zero".to_owned(),
                })
            } else {
                match context
                    .state
                    .graph_store
                    .update_execution_frequency(&id, execution_frequency_hz)
                    .await
                {
                    Ok(true) => None,
                    Ok(false) => Some(ServerMessage::Error {
                        message: format!("Graph document {id} does not exist"),
                    }),
                    Err(error) => {
                        tracing::error!(
                            client_id = context.client_id,
                            %error,
                            %id,
                            "failed to update graph execution frequency"
                        );
                        Some(ServerMessage::Error {
                            message: format!("Failed to update graph execution frequency: {error}"),
                        })
                    }
                }
            }
        }
        ClientMessage::GetNodeDefinitions => Some(ServerMessage::NodeDefinitions {
            definitions: context.state.node_registry.definitions().to_vec(),
        }),
        ClientMessage::GetGraphMetadata => {
            match context.state.graph_store.list_graph_metadata().await {
                Ok(documents) => Some(ServerMessage::GraphMetadata { documents }),
                Err(error) => {
                    tracing::error!(client_id = context.client_id, %error, "failed to load graph metadata");
                    Some(ServerMessage::Error {
                        message: format!("Failed to load graph metadata: {error}"),
                    })
                }
            }
        }
        _ => unreachable!("graphs handler received unsupported message"),
    }
}
