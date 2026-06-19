use flowcore::{NodeId, Workflow};
use std::collections::{HashMap, HashSet, VecDeque};

/// Tracks unsatisfied dependencies for each node.
/// After a node completes, call `mark_satisfied()` for each downstream connection.
/// Call `ready_nodes()` to drain the set of fully-satisfied nodes.
pub struct DependencyTracker {
    /// Unsatisfied predecessor count per node (how many upstream nodes still need to complete)
    unsatisfied: HashMap<NodeId, usize>,
    /// Nodes that have become fully satisfied but not yet pulled
    ready: VecDeque<NodeId>,
    /// Set of nodes already returned as ready (prevents double-scheduling)
    scheduled: HashSet<NodeId>,
}

impl DependencyTracker {
    pub fn new(workflow: &Workflow) -> Self {
        let mut unsatisfied: HashMap<NodeId, usize> = HashMap::new();
        let mut has_incoming: HashSet<NodeId> = HashSet::new();

        for conn in &workflow.connections {
            has_incoming.insert(conn.to_node);
        }

        // Count distinct predecessor nodes per target node
        for conn in &workflow.connections {
            // Use entry for each to_node, but we count distinct from_nodes
            let _count = unsatisfied.entry(conn.to_node).or_insert(0);
            // To avoid double-counting same predecessor, we handle this differently
        }

        // Re-count: for each connection's to_node, find the set of distinct from_nodes
        let mut pred_sets: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
        for conn in &workflow.connections {
            pred_sets
                .entry(conn.to_node)
                .or_default()
                .insert(conn.from_node);
        }

        for (node_id, preds) in &pred_sets {
            unsatisfied.insert(*node_id, preds.len());
        }

        // Nodes without any incoming connections are immediately ready
        let mut ready = VecDeque::new();
        for node_spec in &workflow.nodes {
            if !has_incoming.contains(&node_spec.id) {
                ready.push_back(node_spec.id);
            }
        }

        Self {
            unsatisfied,
            ready,
            scheduled: HashSet::new(),
        }
    }

    /// Called when an upstream node completes.
    /// Decrements the unsatisfied count for the target node.
    /// Returns true if the target node is now fully satisfied.
    pub fn mark_satisfied(&mut self, target_node: NodeId) -> bool {
        if let Some(count) = self.unsatisfied.get_mut(&target_node) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                if !self.scheduled.contains(&target_node) {
                    self.scheduled.insert(target_node);
                    self.ready.push_back(target_node);
                }
                return true;
            }
        }
        false
    }

    /// Drain and return all fully-satisfied nodes that haven't been executed yet.
    pub fn ready_nodes(&mut self) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        while let Some(node_id) = self.ready.pop_front() {
            nodes.push(node_id);
        }
        nodes
    }

    /// Returns true if all nodes have been scheduled.
    pub fn all_scheduled(&self) -> bool {
        self.ready.is_empty()
    }
}
