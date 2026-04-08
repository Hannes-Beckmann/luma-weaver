use std::collections::HashSet;

use shared::{ClientMessage, EventScope, EventSubscription, EventTopic};

use crate::app::FrontendApp;
use crate::state::AppView;

impl FrontendApp {
    /// Reconciles generic event subscriptions and one-shot metadata requests with the current UI state.
    ///
    /// This is called repeatedly from the main app loop, but only sends messages when the desired
    /// subscription set or one-time initialization flags have changed.
    pub(crate) fn sync_event_subscriptions_and_request_initial_data(&mut self) {
        if !self.connection.is_connected() {
            return;
        }

        let desired_subscriptions = self
            .selected_subscriptions()
            .into_iter()
            .collect::<HashSet<_>>();

        if !self.subscriptions.initialized
            || self.subscriptions.active_event_subscriptions != desired_subscriptions
        {
            let added = desired_subscriptions
                .difference(&self.subscriptions.active_event_subscriptions)
                .cloned()
                .collect::<Vec<_>>();
            let removed = self
                .subscriptions
                .active_event_subscriptions
                .difference(&desired_subscriptions)
                .cloned()
                .collect::<Vec<_>>();

            if !removed.is_empty() {
                self.send(ClientMessage::Unsubscribe {
                    subscriptions: removed,
                });
            }

            if !added.is_empty() {
                self.send(ClientMessage::Subscribe {
                    subscriptions: added,
                });
            }

            self.subscriptions.active_event_subscriptions = desired_subscriptions;
            self.subscriptions.initialized = true;
        }

        if !self.subscriptions.metadata_requested_once {
            self.send(ClientMessage::GetGraphMetadata);
            self.subscriptions.metadata_requested_once = true;
        }

        if !self.subscriptions.node_definitions_requested_once {
            self.send(ClientMessage::GetNodeDefinitions);
            self.subscriptions.node_definitions_requested_once = true;
        }

        if !self.subscriptions.running_graphs_requested_once {
            self.send(ClientMessage::GetRuntimeStatuses);
            self.subscriptions.running_graphs_requested_once = true;
        }

        if !self.subscriptions.wled_instances_requested_once {
            self.send(ClientMessage::GetWledInstances);
            self.subscriptions.wled_instances_requested_once = true;
        }

        if !self.subscriptions.mqtt_brokers_requested_once {
            self.send(ClientMessage::GetMqttBrokerConfigs);
            self.subscriptions.mqtt_brokers_requested_once = true;
        }
    }

    /// Reconciles graph-scoped runtime and diagnostics subscriptions for the active editor session.
    ///
    /// Switching graphs or closing diagnostics tears down the obsolete subscriptions and clears the
    /// corresponding cached runtime and diagnostics state.
    pub(crate) fn ensure_runtime_updates_subscription(&mut self) {
        if !self.connection.is_connected() {
            return;
        }

        let desired = if self.ui.active_view == AppView::Editor {
            self.ui.selected_graph_id.clone()
        } else {
            None
        };
        if self.subscriptions.runtime_graph_subscription != desired {
            if let Some(previous) = self.subscriptions.runtime_graph_subscription.take() {
                self.send(ClientMessage::UnsubscribeGraphRuntime { graph_id: previous });
                self.graphs.runtime_node_values.clear();
            }

            if let Some(graph_id) = desired.clone() {
                self.send(ClientMessage::SubscribeGraphRuntime {
                    graph_id: graph_id.clone(),
                });
                self.subscriptions.runtime_graph_subscription = Some(graph_id);
                self.graphs.runtime_node_values.clear();
            }
        }

        let desired_diagnostics_graph = if self.ui.active_view == AppView::Editor {
            self.ui.selected_graph_id.clone()
        } else {
            None
        };
        if self.subscriptions.diagnostics_graph_subscription != desired_diagnostics_graph {
            if let Some(previous) = self.subscriptions.diagnostics_graph_subscription.take() {
                self.send(ClientMessage::UnsubscribeGraphDiagnostics { graph_id: previous });
                self.graphs.node_diagnostic_summaries.clear();
            }
            if let Some(graph_id) = desired_diagnostics_graph.clone() {
                self.send(ClientMessage::SubscribeGraphDiagnostics {
                    graph_id: graph_id.clone(),
                });
                self.subscriptions.diagnostics_graph_subscription = Some(graph_id);
            }
        }

        let desired_node_diagnostics = if self.ui.active_view == AppView::Editor {
            match (
                self.ui.selected_graph_id.clone(),
                self.ui.diagnostics_window_node_id.clone(),
            ) {
                (Some(graph_id), Some(node_id)) => Some((graph_id, node_id)),
                _ => None,
            }
        } else {
            None
        };
        if self.subscriptions.diagnostics_node_subscription != desired_node_diagnostics {
            if let Some((graph_id, node_id)) =
                self.subscriptions.diagnostics_node_subscription.take()
            {
                self.send(ClientMessage::UnsubscribeNodeDiagnostics { graph_id, node_id });
            }
            if let Some((graph_id, node_id)) = desired_node_diagnostics.clone() {
                self.send(ClientMessage::SubscribeNodeDiagnostics {
                    graph_id: graph_id.clone(),
                    node_id: node_id.clone(),
                });
                self.subscriptions.diagnostics_node_subscription = Some((graph_id, node_id));
            }
        }
    }

    /// Builds the desired generic event-subscription set from the current UI toggles and selection.
    pub(crate) fn selected_subscriptions(&self) -> Vec<EventSubscription> {
        let mut topics = Vec::new();

        if self.subscriptions.subscribe_connection {
            topics.push(EventTopic::Connection);
        }

        if self.subscriptions.subscribe_ping {
            topics.push(EventTopic::Ping);
        }

        if self.subscriptions.subscribe_name {
            topics.push(EventTopic::Name);
        }

        if self.subscriptions.subscribe_graph_metadata_changed {
            topics.push(EventTopic::GraphMetadataChanged);
        }

        let mut subscriptions = Vec::new();
        let mut scopes = vec![EventScope::Global];
        if let Some(graph_id) = self.ui.selected_graph_id.clone() {
            scopes.push(EventScope::Graph { graph_id });
        }

        for topic in topics {
            for scope in &scopes {
                subscriptions.push(EventSubscription {
                    topic,
                    scope: scope.clone(),
                });
            }
        }

        subscriptions
    }
}

/// Formats a subscription list for status text and debug logging.
pub(crate) fn format_subscriptions(subscriptions: &[EventSubscription]) -> String {
    if subscriptions.is_empty() {
        return "none".to_owned();
    }

    subscriptions
        .iter()
        .map(format_subscription)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Formats a single subscription as `<topic>@<scope>`.
fn format_subscription(subscription: &EventSubscription) -> String {
    let scope = match &subscription.scope {
        EventScope::Global => "global".to_owned(),
        EventScope::Graph { graph_id } => format!("graph:{graph_id}"),
        EventScope::Node { graph_id, node_id } => format!("node:{graph_id}/{node_id}"),
        EventScope::Element {
            graph_id,
            node_id,
            element_id,
        } => format!("element:{graph_id}/{node_id}/{element_id}"),
    };

    format!("{}@{scope}", subscription.topic.label())
}
