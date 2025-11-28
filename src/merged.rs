use std::collections::HashMap;
use serde::Serialize;
use crate::node::{Node, NodeRef};

/// Description of a node's split logic, used for comparing nodes for equality
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NodeInfo {
    Leaf { word: String },
    Repeat { word: String },
    Split { letter: char },
    SoftSplit { test_letter: char, requirement_letter: char },
    FirstLetterSplit { letter: char },
    SoftFirstLetterSplit { test_letter: char, requirement_letter: char },
    LastLetterSplit { letter: char },
    SoftLastLetterSplit { test_letter: char, requirement_letter: char },
    SoftMirrorPosSplit {
        test_letter: char,
        test_index: u8,
        test_from_end: bool,
        requirement_index: u8,
        requirement_from_end: bool,
    },
    SoftDoubleLetterSplit { test_letter: char, requirement_letter: char },
}

impl NodeInfo {
    /// Extract node info from a Node, ignoring children
    fn from_node(node: &Node) -> Self {
        match node {
            Node::Leaf(word) => NodeInfo::Leaf { word: word.clone() },
            Node::Repeat { word, .. } => NodeInfo::Repeat { word: word.clone() },
            Node::Split { letter, .. } => NodeInfo::Split { letter: *letter },
            Node::SoftSplit { test_letter, requirement_letter, .. } => {
                NodeInfo::SoftSplit {
                    test_letter: *test_letter,
                    requirement_letter: *requirement_letter,
                }
            }
            Node::FirstLetterSplit { letter, .. } => {
                NodeInfo::FirstLetterSplit { letter: *letter }
            }
            Node::SoftFirstLetterSplit { test_letter, requirement_letter, .. } => {
                NodeInfo::SoftFirstLetterSplit {
                    test_letter: *test_letter,
                    requirement_letter: *requirement_letter,
                }
            }
            Node::LastLetterSplit { letter, .. } => {
                NodeInfo::LastLetterSplit { letter: *letter }
            }
            Node::SoftLastLetterSplit { test_letter, requirement_letter, .. } => {
                NodeInfo::SoftLastLetterSplit {
                    test_letter: *test_letter,
                    requirement_letter: *requirement_letter,
                }
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                ..
            } => {
                NodeInfo::SoftMirrorPosSplit {
                    test_letter: *test_letter,
                    test_index: *test_index,
                    test_from_end: *test_from_end,
                    requirement_index: *requirement_index,
                    requirement_from_end: *requirement_from_end,
                }
            }
            Node::SoftDoubleLetterSplit { test_letter, requirement_letter, .. } => {
                NodeInfo::SoftDoubleLetterSplit {
                    test_letter: *test_letter,
                    requirement_letter: *requirement_letter,
                }
            }
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
                        Node::Split { yes, no, .. }
                        | Node::SoftSplit { yes, no, .. }
                        | Node::FirstLetterSplit { yes, no, .. }
                        | Node::SoftFirstLetterSplit { yes, no, .. }
                        | Node::LastLetterSplit { yes, no, .. }
                        | Node::SoftLastLetterSplit { yes, no, .. }
                        | Node::SoftMirrorPosSplit { yes, no, .. }
                        | Node::SoftDoubleLetterSplit { yes, no, .. } => {
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
