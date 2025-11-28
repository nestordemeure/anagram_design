use crate::node::Node;

pub fn format_tree(node: &Node) -> String {
    // Helper to capitalize the first letter of a word
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    // Display helper: show question letters in uppercase for clarity in ASCII trees
    fn display_letter(c: char) -> char {
        c.to_ascii_uppercase()
    }

    fn describe_pos(from_end: bool, idx: u8) -> String {
        match (from_end, idx) {
            (false, 1) => "first".to_string(),
            (false, 2) => "second".to_string(),
            (false, 3) => "third".to_string(),
            (true, 1) => "last".to_string(),
            (true, 2) => "second-to-last".to_string(),
            (true, 3) => "third-to-last".to_string(),
            _ => format!("pos {}", idx),
        }
    }

    // Render a No branch that diverges sideways from the main spine.
    fn render_no_branch(node: &Node, prefix: &str, out: &mut String) {
        match node {
            Node::Leaf(w) => {
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&capitalize_first(w));
                out.push('\n');
            }
            Node::Repeat { word, no } => {
                out.push_str(prefix);
                out.push_str("└─ No: Repeat ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str("...\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                render_yes_final(&Node::Leaf(word.clone()), &child_prefix, out);
            }
            Node::Split { letter, yes, no } => {
                // No branch that contains another split
                out.push_str(prefix);
                out.push_str("└─ No: Contains '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // The no-branch's children are indented with "│   "
                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                // The yes branch of this nested split uses └─ (it's the final item in this branch)
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // No branch that contains a soft split
                out.push_str(prefix);
                out.push_str("└─ No: Contains '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No contain '");
                out.push(display_letter(*requirement_letter));
                out.push_str("')\n");

                // The no-branch's children are indented with "│   "
                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);

                // The yes branch of this nested split uses └─ (it's the final item in this branch)
                render_yes_final(yes, &child_prefix, out);
            }
            Node::FirstLetterSplit { letter, yes, no } => {
                // No branch that contains a first letter split
                out.push_str(prefix);
                out.push_str("└─ No: First letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftFirstLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // No branch that contains a soft first letter split
                out.push_str(prefix);
                out.push_str("└─ No: First letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second)\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // No branch that contains a last letter split
                out.push_str(prefix);
                out.push_str("└─ No: Last letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftLastLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // No branch that contains a soft last letter split
                out.push_str(prefix);
                out.push_str("└─ No: Last letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second-to-last)\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::SoftDoubleLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("└─ No: Double '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No double '");
                out.push(display_letter(*requirement_letter));
                out.push_str("')\n");

                let child_prefix = format!("{}   ", prefix);
                render_no_branch(no, &format!("{}│", child_prefix), out);
                render_yes_final(yes, &child_prefix, out);
            }
        }
    }

    // Render a final Yes item (uses └─ marker for leaves/repeats, continues spine for splits)
    fn render_yes_final(node: &Node, prefix: &str, out: &mut String) {
        match node {
            Node::Leaf(w) => {
                out.push_str(prefix);
                out.push_str("└─ ");
                out.push_str(&capitalize_first(w));
                out.push('\n');
            }
            Node::Repeat { word, no } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Repeat ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str("...\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(&Node::Leaf(word.clone()), prefix, out);
            }
            Node::Split { letter, yes, no } => {
                // For a split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // For a soft split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No contain '");
                out.push(display_letter(*requirement_letter));
                out.push_str("')\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::FirstLetterSplit { letter, yes, no } => {
                // For a first letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftFirstLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // For a soft first letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second)\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // For a last letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftLastLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // For a soft last letter split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second-to-last)\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
            Node::SoftDoubleLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str("Double '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No double '");
                out.push(display_letter(*requirement_letter));
                out.push_str("')\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(yes, prefix, out);
            }
        }
    }

    // Render the main Yes spine; No branches jut out to the side.
    fn render_spine(node: &Node, prefix: &str, is_final: bool, out: &mut String) {
        match node {
            Node::Leaf(w) => {
                let connector = if is_final { "└─ " } else { "├─ " };
                out.push_str(prefix);
                out.push_str(connector);
                out.push_str(&capitalize_first(w));
                out.push('\n');
            }
            Node::Repeat { word, no } => {
                out.push_str(prefix);
                out.push_str("Repeat ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str(", ");
                out.push_str(&capitalize_first(word));
                out.push_str("...\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(&Node::Leaf(word.clone()), prefix, is_final, out);
            }
            Node::Split { letter, yes, no } => {
                // Print the question
                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // Print the question for soft split
                out.push_str(prefix);
                out.push_str("Contains '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No contain '");
                out.push(display_letter(*requirement_letter));
                out.push_str("')\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::FirstLetterSplit { letter, yes, no } => {
                // Print the question for first letter split
                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftFirstLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // Print the question for soft first letter split
                out.push_str(prefix);
                out.push_str("First letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second)\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::LastLetterSplit { letter, yes, no } => {
                // Print the question for last letter split
                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(display_letter(*letter));
                out.push_str("'?\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftLastLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                // Print the question for soft last letter split
                out.push_str(prefix);
                out.push_str("Last letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have '");
                out.push(display_letter(*requirement_letter));
                out.push_str("' second-to-last)\n");

                // No branch diverges sideways
                render_no_branch(no, &format!("{}│", prefix), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftMirrorPosSplit {
                test_letter,
                test_index,
                test_from_end,
                requirement_index,
                requirement_from_end,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str(&describe_pos(*test_from_end, *test_index));
                out.push_str(" letter '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No have it ");
                out.push_str(&describe_pos(*requirement_from_end, *requirement_index));
                out.push_str(")\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(yes, prefix, is_final, out);
            }
            Node::SoftDoubleLetterSplit {
                test_letter,
                requirement_letter,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("Double '");
                out.push(display_letter(*test_letter));
                out.push_str("'? (all No double '");
                out.push(display_letter(*requirement_letter));
                out.push_str("')\n");

                render_no_branch(no, &format!("{}│", prefix), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(yes, prefix, is_final, out);
            }
        }
    }

    let mut out = String::new();
    render_spine(node, "", true, &mut out);
    out
}
