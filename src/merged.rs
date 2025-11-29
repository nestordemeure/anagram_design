use std::collections::HashMap;
use serde::Serialize;
use crate::node::{Node, NodeRef, Position};

/// Description of a node's split logic, used for comparing nodes for equality
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NodeInfo {
    Leaf { word: String },
    Repeat { word: String },
    PositionalSplit {
        test_letter: char,
        test_position: Position,
        requirement_letter: char,
        requirement_position: Position,
    },
}

impl NodeInfo {
    /// Extract node info from a Node, ignoring children
    fn from_node(node: &Node) -> Self {
        match node {
            Node::Leaf(word) => NodeInfo::Leaf { word: word.clone() },
            Node::Repeat { word, .. } => NodeInfo::Repeat { word: word.clone() },
            Node::PositionalSplit {
                test_letter,
                test_position,
                requirement_letter,
                requirement_position,
                ..
            } => NodeInfo::PositionalSplit {
                test_letter: *test_letter,
                test_position: *test_position,
                requirement_letter: *requirement_letter,
                requirement_position: *requirement_position,
            },
        }
    }
}

/// A single option in a merged node (one possible split + its merged children)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedOption {
    pub info: NodeInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes_branch: Option<Box<MergedNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_branch: Option<Box<MergedNode>>,
}

/// A merged node that may have multiple options (when trees differ)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedNode {
    /// If options.len() == 1, all trees agree (unanimous)
    /// If options.len() > 1, user can choose (choice node)
    pub options: Vec<MergedOption>,
}

impl MergedNode {
    /// Merge multiple trees into a single merged tree structure
    pub fn merge(trees: &[NodeRef]) -> Self {
        if trees.is_empty() {
            return MergedNode { options: vec![] };
        }

        // Group trees by their root node info
        let mut groups: HashMap<NodeInfo, Vec<NodeRef>> = HashMap::new();
        for tree in trees {
            let info = NodeInfo::from_node(tree);
            groups.entry(info).or_insert_with(Vec::new).push(tree.clone());
        }

        // For each group, merge the children
        let mut options: Vec<MergedOption> = groups
            .into_iter()
            .map(|(info, group_trees)| {
                // Extract yes and no branches from all trees in this group
                let mut yes_branches = Vec::new();
                let mut no_branches = Vec::new();

                for tree in &group_trees {
                    match &**tree {
                        Node::Leaf(_) => {
                            // Leaves have no children
                        }
                        Node::Repeat { no, .. } => {
                            no_branches.push(no.clone());
                        }
                        Node::PositionalSplit { yes, no, .. } => {
                            yes_branches.push(yes.clone());
                            no_branches.push(no.clone());
                        }
                    }
                }

                // Recursively merge children
                let yes_branch = if yes_branches.is_empty() {
                    None
                } else {
                    Some(Box::new(MergedNode::merge(&yes_branches)))
                };

                let no_branch = if no_branches.is_empty() {
                    None
                } else {
                    Some(Box::new(MergedNode::merge(&no_branches)))
                };

                MergedOption {
                    info,
                    yes_branch,
                    no_branch,
                }
            })
            .collect();

        // Sort options for consistent ordering (first option should be the first tree)
        // We'll sort by the NodeInfo Debug representation for determinism
        options.sort_by(|a, b| format!("{:?}", a.info).cmp(&format!("{:?}", b.info)));

        MergedNode { options }
    }

    /// Check if this is a choice node (multiple options)
    pub fn is_choice(&self) -> bool {
        self.options.len() > 1
    }

    /// Check if this is a leaf node (no children)
    pub fn is_leaf(&self) -> bool {
        self.options.len() == 1
            && matches!(self.options[0].info, NodeInfo::Leaf { .. })
    }
}
