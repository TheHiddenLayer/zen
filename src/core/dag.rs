//! Task DAG (Directed Acyclic Graph) for dependency management.
//!
//! This module provides the TaskDAG structure that represents task dependencies
//! as a directed acyclic graph, enabling parallel execution of independent tasks.

use crate::core::task::{Task, TaskId};
use crate::error::{Error, Result};
use petgraph::algo::is_cyclic_directed;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Type of dependency between tasks.
///
/// Dependencies capture the relationship between tasks and why one
/// task must complete before another can start.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum DependencyType {
    /// Task B requires output of Task A (data flow dependency).
    DataDependency,
    /// Task B modifies files that Task A also modifies.
    FileDependency {
        /// List of files that both tasks touch.
        files: Vec<PathBuf>,
    },
    /// AI-inferred semantic dependency.
    SemanticDependency {
        /// Reason for the semantic dependency.
        reason: String,
    },
}

impl Default for DependencyType {
    fn default() -> Self {
        Self::DataDependency
    }
}

impl std::fmt::Display for DependencyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyType::DataDependency => write!(f, "data"),
            DependencyType::FileDependency { files } => {
                write!(f, "files: {}", files.len())
            }
            DependencyType::SemanticDependency { reason } => {
                write!(f, "semantic: {}", reason)
            }
        }
    }
}

/// The task dependency graph.
///
/// TaskDAG uses petgraph's DiGraph to represent task dependencies.
/// Nodes are tasks, and edges represent dependencies with metadata
/// about the type of dependency.
pub struct TaskDAG {
    /// The underlying directed graph.
    graph: DiGraph<Task, DependencyType>,
    /// Index mapping from TaskId to NodeIndex for fast lookups.
    task_index: HashMap<TaskId, NodeIndex>,
}

impl TaskDAG {
    /// Create a new empty TaskDAG.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            task_index: HashMap::new(),
        }
    }

    /// Add a task to the DAG.
    ///
    /// Returns the NodeIndex for the added task.
    /// If the task already exists (same TaskId), returns the existing NodeIndex.
    pub fn add_task(&mut self, task: Task) -> NodeIndex {
        // Check if task already exists
        if let Some(&index) = self.task_index.get(&task.id) {
            return index;
        }

        let id = task.id;
        let index = self.graph.add_node(task);
        self.task_index.insert(id, index);
        index
    }

    /// Add a dependency between two tasks.
    ///
    /// The dependency indicates that `from` must complete before `to` can start.
    /// This method validates that adding the dependency won't create a cycle.
    ///
    /// # Arguments
    /// * `from` - The task that must complete first (dependency)
    /// * `to` - The task that depends on `from`
    /// * `dep_type` - The type of dependency
    ///
    /// # Errors
    /// Returns an error if:
    /// - Either task is not found in the DAG
    /// - Adding the edge would create a cycle
    pub fn add_dependency(
        &mut self,
        from: &TaskId,
        to: &TaskId,
        dep_type: DependencyType,
    ) -> Result<()> {
        let from_index = self
            .task_index
            .get(from)
            .ok_or_else(|| Error::Validation(format!("Task {} not found in DAG", from)))?;

        let to_index = self
            .task_index
            .get(to)
            .ok_or_else(|| Error::Validation(format!("Task {} not found in DAG", to)))?;

        // Temporarily add the edge to check for cycles
        let edge = self.graph.add_edge(*from_index, *to_index, dep_type);

        // Check if this creates a cycle
        if is_cyclic_directed(&self.graph) {
            // Remove the edge we just added
            self.graph.remove_edge(edge);
            return Err(Error::Validation(format!(
                "Adding dependency from {} to {} would create a cycle",
                from, to
            )));
        }

        Ok(())
    }

    /// Get a reference to a task by its ID.
    pub fn get_task(&self, id: &TaskId) -> Option<&Task> {
        self.task_index
            .get(id)
            .and_then(|&index| self.graph.node_weight(index))
    }

    /// Get a mutable reference to a task by its ID.
    pub fn get_task_mut(&mut self, id: &TaskId) -> Option<&mut Task> {
        if let Some(&index) = self.task_index.get(id) {
            self.graph.node_weight_mut(index)
        } else {
            None
        }
    }

    /// Get the NodeIndex for a task by its ID.
    pub fn get_node_index(&self, id: &TaskId) -> Option<NodeIndex> {
        self.task_index.get(id).copied()
    }

    /// Get the number of tasks in the DAG.
    pub fn task_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get the number of dependencies (edges) in the DAG.
    pub fn dependency_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Check if a dependency exists between two tasks.
    pub fn has_dependency(&self, from: &TaskId, to: &TaskId) -> bool {
        if let (Some(&from_idx), Some(&to_idx)) =
            (self.task_index.get(from), self.task_index.get(to))
        {
            self.graph.find_edge(from_idx, to_idx).is_some()
        } else {
            false
        }
    }

    /// Get the dependency type between two tasks, if one exists.
    pub fn get_dependency(&self, from: &TaskId, to: &TaskId) -> Option<&DependencyType> {
        let from_idx = self.task_index.get(from)?;
        let to_idx = self.task_index.get(to)?;
        let edge = self.graph.find_edge(*from_idx, *to_idx)?;
        self.graph.edge_weight(edge)
    }

    /// Get all tasks that the given task depends on (predecessors).
    pub fn get_dependencies(&self, id: &TaskId) -> Vec<&Task> {
        if let Some(&index) = self.task_index.get(id) {
            self.graph
                .neighbors_directed(index, petgraph::Direction::Incoming)
                .filter_map(|neighbor| self.graph.node_weight(neighbor))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all tasks that depend on the given task (successors).
    pub fn get_dependents(&self, id: &TaskId) -> Vec<&Task> {
        if let Some(&index) = self.task_index.get(id) {
            self.graph
                .neighbors_directed(index, petgraph::Direction::Outgoing)
                .filter_map(|neighbor| self.graph.node_weight(neighbor))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all tasks in the DAG.
    pub fn all_tasks(&self) -> Vec<&Task> {
        self.graph.node_weights().collect()
    }

    /// Check if the DAG is empty.
    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Check if the DAG contains a task.
    pub fn contains_task(&self, id: &TaskId) -> bool {
        self.task_index.contains_key(id)
    }

    /// Get the underlying graph for advanced operations.
    ///
    /// This is useful for algorithms that need direct graph access.
    pub fn graph(&self) -> &DiGraph<Task, DependencyType> {
        &self.graph
    }
}

impl Default for TaskDAG {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TaskDAG {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskDAG")
            .field("tasks", &self.task_count())
            .field("dependencies", &self.dependency_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a test task
    fn test_task(name: &str) -> Task {
        Task::new(name, &format!("{} description", name))
    }

    // DependencyType tests

    #[test]
    fn test_dependency_type_default() {
        let dep = DependencyType::default();
        assert!(matches!(dep, DependencyType::DataDependency));
    }

    #[test]
    fn test_dependency_type_display_data() {
        let dep = DependencyType::DataDependency;
        assert_eq!(format!("{}", dep), "data");
    }

    #[test]
    fn test_dependency_type_display_file() {
        let dep = DependencyType::FileDependency {
            files: vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")],
        };
        assert_eq!(format!("{}", dep), "files: 2");
    }

    #[test]
    fn test_dependency_type_display_semantic() {
        let dep = DependencyType::SemanticDependency {
            reason: "API needs model".to_string(),
        };
        assert_eq!(format!("{}", dep), "semantic: API needs model");
    }

    #[test]
    fn test_dependency_type_serialization_data() {
        let dep = DependencyType::DataDependency;
        let json = serde_json::to_string(&dep).unwrap();
        assert!(json.contains("data_dependency"));
        let parsed: DependencyType = serde_json::from_str(&json).unwrap();
        assert_eq!(dep, parsed);
    }

    #[test]
    fn test_dependency_type_serialization_file() {
        let dep = DependencyType::FileDependency {
            files: vec![PathBuf::from("src/main.rs")],
        };
        let json = serde_json::to_string(&dep).unwrap();
        assert!(json.contains("file_dependency"));
        assert!(json.contains("src/main.rs"));
        let parsed: DependencyType = serde_json::from_str(&json).unwrap();
        assert_eq!(dep, parsed);
    }

    #[test]
    fn test_dependency_type_serialization_semantic() {
        let dep = DependencyType::SemanticDependency {
            reason: "schema before API".to_string(),
        };
        let json = serde_json::to_string(&dep).unwrap();
        assert!(json.contains("semantic_dependency"));
        assert!(json.contains("schema before API"));
        let parsed: DependencyType = serde_json::from_str(&json).unwrap();
        assert_eq!(dep, parsed);
    }

    // TaskDAG basic tests

    #[test]
    fn test_dag_new() {
        let dag = TaskDAG::new();
        assert!(dag.is_empty());
        assert_eq!(dag.task_count(), 0);
        assert_eq!(dag.dependency_count(), 0);
    }

    #[test]
    fn test_dag_default() {
        let dag = TaskDAG::default();
        assert!(dag.is_empty());
    }

    #[test]
    fn test_dag_debug() {
        let dag = TaskDAG::new();
        let debug = format!("{:?}", dag);
        assert!(debug.contains("TaskDAG"));
        assert!(debug.contains("tasks"));
        assert!(debug.contains("dependencies"));
    }

    // Task addition tests

    #[test]
    fn test_dag_add_task() {
        let mut dag = TaskDAG::new();
        let task = test_task("task-a");
        let id = task.id;

        let index = dag.add_task(task);

        assert!(!dag.is_empty());
        assert_eq!(dag.task_count(), 1);
        assert!(dag.contains_task(&id));
        assert!(dag.get_node_index(&id).is_some());
        assert_eq!(dag.get_node_index(&id), Some(index));
    }

    #[test]
    fn test_dag_add_task_is_retrievable() {
        let mut dag = TaskDAG::new();
        let task = test_task("task-a");
        let id = task.id;

        dag.add_task(task);

        let retrieved = dag.get_task(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "task-a");
    }

    #[test]
    fn test_dag_add_task_duplicate() {
        let mut dag = TaskDAG::new();
        let task = test_task("task-a");
        let id = task.id;

        let index1 = dag.add_task(task.clone());
        let index2 = dag.add_task(task);

        // Same task added twice should return the same index
        assert_eq!(index1, index2);
        assert_eq!(dag.task_count(), 1);
        assert!(dag.contains_task(&id));
    }

    #[test]
    fn test_dag_add_multiple_tasks() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");

        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);

        assert_eq!(dag.task_count(), 3);
        assert!(dag.contains_task(&id_a));
        assert!(dag.contains_task(&id_b));
        assert!(dag.contains_task(&id_c));
    }

    #[test]
    fn test_dag_get_task_not_found() {
        let dag = TaskDAG::new();
        let id = TaskId::new();
        assert!(dag.get_task(&id).is_none());
    }

    #[test]
    fn test_dag_get_task_mut() {
        let mut dag = TaskDAG::new();
        let task = test_task("task-a");
        let id = task.id;

        dag.add_task(task);

        // Mutate the task
        if let Some(task) = dag.get_task_mut(&id) {
            task.mark_ready();
        }

        // Verify mutation persisted
        let task = dag.get_task(&id).unwrap();
        assert!(matches!(
            task.status,
            crate::core::task::TaskStatus::Ready
        ));
    }

    #[test]
    fn test_dag_all_tasks() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");

        dag.add_task(task_a);
        dag.add_task(task_b);

        let all = dag.all_tasks();
        assert_eq!(all.len(), 2);
    }

    // Dependency tests

    #[test]
    fn test_dag_add_dependency() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        dag.add_task(task_a);
        dag.add_task(task_b);

        let result = dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency);

        assert!(result.is_ok());
        assert_eq!(dag.dependency_count(), 1);
        assert!(dag.has_dependency(&id_a, &id_b));
    }

    #[test]
    fn test_dag_add_dependency_preserves_type() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        dag.add_task(task_a);
        dag.add_task(task_b);

        dag.add_dependency(
            &id_a,
            &id_b,
            DependencyType::FileDependency {
                files: vec![PathBuf::from("model.rs")],
            },
        )
        .unwrap();

        let dep = dag.get_dependency(&id_a, &id_b);
        assert!(dep.is_some());
        assert!(matches!(dep.unwrap(), DependencyType::FileDependency { .. }));
    }

    #[test]
    fn test_dag_add_dependency_from_not_found() {
        let mut dag = TaskDAG::new();
        let task_b = test_task("task-b");
        let id_a = TaskId::new(); // Not in DAG
        let id_b = task_b.id;

        dag.add_task(task_b);

        let result = dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_dag_add_dependency_to_not_found() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let id_a = task_a.id;
        let id_b = TaskId::new(); // Not in DAG

        dag.add_task(task_a);

        let result = dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_dag_has_dependency_false() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        dag.add_task(task_a);
        dag.add_task(task_b);

        assert!(!dag.has_dependency(&id_a, &id_b));
        assert!(!dag.has_dependency(&id_b, &id_a));
    }

    #[test]
    fn test_dag_get_dependency_not_found() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        dag.add_task(task_a);
        dag.add_task(task_b);

        assert!(dag.get_dependency(&id_a, &id_b).is_none());
    }

    // Cycle detection tests

    #[test]
    fn test_dag_cycle_detection_self_loop() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let id_a = task_a.id;

        dag.add_task(task_a);

        let result = dag.add_dependency(&id_a, &id_a, DependencyType::DataDependency);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("cycle"));
        assert_eq!(dag.dependency_count(), 0);
    }

    #[test]
    fn test_dag_cycle_detection_two_nodes() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        dag.add_task(task_a);
        dag.add_task(task_b);

        // A -> B
        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();

        // B -> A would create cycle
        let result = dag.add_dependency(&id_b, &id_a, DependencyType::DataDependency);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("cycle"));
        assert_eq!(dag.dependency_count(), 1);
    }

    #[test]
    fn test_dag_cycle_detection_three_nodes() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);

        // A -> B -> C
        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_b, &id_c, DependencyType::DataDependency)
            .unwrap();

        // C -> A would create cycle
        let result = dag.add_dependency(&id_c, &id_a, DependencyType::DataDependency);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("cycle"));
        assert_eq!(dag.dependency_count(), 2);
    }

    #[test]
    fn test_dag_valid_chain_no_cycle() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let task_d = test_task("task-d");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;
        let id_d = task_d.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);
        dag.add_task(task_d);

        // A -> B -> C -> D (valid chain)
        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_b, &id_c, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_c, &id_d, DependencyType::DataDependency)
            .unwrap();

        assert_eq!(dag.dependency_count(), 3);
    }

    #[test]
    fn test_dag_diamond_pattern_no_cycle() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let task_d = test_task("task-d");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;
        let id_d = task_d.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);
        dag.add_task(task_d);

        //     A
        //    / \
        //   B   C
        //    \ /
        //     D
        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_a, &id_c, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_b, &id_d, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_c, &id_d, DependencyType::DataDependency)
            .unwrap();

        assert_eq!(dag.dependency_count(), 4);
    }

    // Dependencies and dependents tests

    #[test]
    fn test_dag_get_dependencies() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);

        // A -> C, B -> C (C depends on A and B)
        dag.add_dependency(&id_a, &id_c, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_b, &id_c, DependencyType::DataDependency)
            .unwrap();

        let deps = dag.get_dependencies(&id_c);
        assert_eq!(deps.len(), 2);

        let dep_names: Vec<&str> = deps.iter().map(|t| t.name.as_str()).collect();
        assert!(dep_names.contains(&"task-a"));
        assert!(dep_names.contains(&"task-b"));
    }

    #[test]
    fn test_dag_get_dependencies_none() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let id_a = task_a.id;

        dag.add_task(task_a);

        let deps = dag.get_dependencies(&id_a);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_dag_get_dependencies_not_in_dag() {
        let dag = TaskDAG::new();
        let id = TaskId::new();

        let deps = dag.get_dependencies(&id);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_dag_get_dependents() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);

        // A -> B, A -> C (B and C depend on A)
        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(&id_a, &id_c, DependencyType::DataDependency)
            .unwrap();

        let dependents = dag.get_dependents(&id_a);
        assert_eq!(dependents.len(), 2);

        let dep_names: Vec<&str> = dependents.iter().map(|t| t.name.as_str()).collect();
        assert!(dep_names.contains(&"task-b"));
        assert!(dep_names.contains(&"task-c"));
    }

    #[test]
    fn test_dag_get_dependents_none() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let id_a = task_a.id;

        dag.add_task(task_a);

        let dependents = dag.get_dependents(&id_a);
        assert!(dependents.is_empty());
    }

    // Index consistency tests

    #[test]
    fn test_dag_index_consistency_after_multiple_adds() {
        let mut dag = TaskDAG::new();
        let mut ids = Vec::new();

        for i in 0..10 {
            let task = test_task(&format!("task-{}", i));
            ids.push(task.id);
            dag.add_task(task);
        }

        // Verify all tasks are retrievable
        for (i, id) in ids.iter().enumerate() {
            let task = dag.get_task(id);
            assert!(task.is_some());
            assert_eq!(task.unwrap().name, format!("task-{}", i));
        }
    }

    #[test]
    fn test_dag_node_index_returned_matches_lookup() {
        let mut dag = TaskDAG::new();
        let task = test_task("task-a");
        let id = task.id;

        let returned_index = dag.add_task(task);
        let looked_up_index = dag.get_node_index(&id);

        assert_eq!(Some(returned_index), looked_up_index);
    }

    // Graph access tests

    #[test]
    fn test_dag_graph_accessor() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();

        let graph = dag.graph();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    // Edge case tests

    #[test]
    fn test_dag_contains_task_false() {
        let dag = TaskDAG::new();
        let id = TaskId::new();
        assert!(!dag.contains_task(&id));
    }

    #[test]
    fn test_dag_get_node_index_not_found() {
        let dag = TaskDAG::new();
        let id = TaskId::new();
        assert!(dag.get_node_index(&id).is_none());
    }

    #[test]
    fn test_dag_multiple_dependency_types() {
        let mut dag = TaskDAG::new();
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let task_d = test_task("task-d");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;
        let id_d = task_d.id;

        dag.add_task(task_a);
        dag.add_task(task_b);
        dag.add_task(task_c);
        dag.add_task(task_d);

        dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
            .unwrap();
        dag.add_dependency(
            &id_b,
            &id_c,
            DependencyType::FileDependency {
                files: vec![PathBuf::from("shared.rs")],
            },
        )
        .unwrap();
        dag.add_dependency(
            &id_c,
            &id_d,
            DependencyType::SemanticDependency {
                reason: "tests need implementation".to_string(),
            },
        )
        .unwrap();

        assert_eq!(dag.dependency_count(), 3);

        assert!(matches!(
            dag.get_dependency(&id_a, &id_b),
            Some(DependencyType::DataDependency)
        ));
        assert!(matches!(
            dag.get_dependency(&id_b, &id_c),
            Some(DependencyType::FileDependency { .. })
        ));
        assert!(matches!(
            dag.get_dependency(&id_c, &id_d),
            Some(DependencyType::SemanticDependency { .. })
        ));
    }
}
