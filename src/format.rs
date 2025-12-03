use crate::node::{Node, Position};

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
    const fn display_letter(c: char) -> char {
        c.to_ascii_uppercase()
    }

    // Format a position description
    fn format_position_question(
        test_letter: char,
        test_position: &Position,
        requirement_letter: char,
        requirement_position: &Position,
    ) -> String {
        let test_letter_upper = display_letter(test_letter);
        let req_letter_upper = display_letter(requirement_letter);

        // Hard split: test and requirement are the same
        if test_letter == requirement_letter && test_position == requirement_position {
            match test_position {
                Position::Contains => format!("Contains '{test_letter_upper}'?"),
                Position::First => format!("First letter '{test_letter_upper}'?"),
                Position::Second => format!("Second letter '{test_letter_upper}'?"),
                Position::Third => format!("Third letter '{test_letter_upper}'?"),
                Position::ThirdToLast => format!("Third-to-last letter '{test_letter_upper}'?"),
                Position::SecondToLast => format!("Second-to-last letter '{test_letter_upper}'?"),
                Position::Last => format!("Last letter '{test_letter_upper}'?"),
                Position::Double => format!("Double '{test_letter_upper}'?"),
                Position::Triple => format!("Triple '{test_letter_upper}'?"),
            }
        } else {
            // Soft split: different test and requirement
            let test_desc = match test_position {
                Position::Contains => format!("Contains '{test_letter_upper}'?"),
                Position::First => format!("First letter '{test_letter_upper}'?"),
                Position::Second => format!("Second letter '{test_letter_upper}'?"),
                Position::Third => format!("Third letter '{test_letter_upper}'?"),
                Position::ThirdToLast => format!("Third-to-last letter '{test_letter_upper}'?"),
                Position::SecondToLast => format!("Second-to-last letter '{test_letter_upper}'?"),
                Position::Last => format!("Last letter '{test_letter_upper}'?"),
                Position::Double => format!("Double '{test_letter_upper}'?"),
                Position::Triple => format!("Triple '{test_letter_upper}'?"),
            };

            let req_desc = match requirement_position {
                Position::Contains => format!("all No contain '{req_letter_upper}'"),
                Position::First => format!("all No have '{req_letter_upper}' first"),
                Position::Second => format!("all No have '{req_letter_upper}' second"),
                Position::Third => format!("all No have '{req_letter_upper}' third"),
                Position::ThirdToLast => format!("all No have '{req_letter_upper}' third-to-last"),
                Position::SecondToLast => format!("all No have '{req_letter_upper}' second-to-last"),
                Position::Last => format!("all No have '{req_letter_upper}' last"),
                Position::Double => format!("all No double '{req_letter_upper}'"),
                Position::Triple => format!("all No triple '{req_letter_upper}'"),
            };

            format!("{test_desc} ({req_desc})")
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

                let child_prefix = format!("{prefix}   ");
                render_no_branch(no, &format!("{child_prefix}│"), out);

                render_yes_final(&Node::Leaf(word.clone()), &child_prefix, out);
            }
            Node::PositionalSplit {
                test_letter,
                test_position,
                requirement_letter,
                requirement_position,
                yes,
                no,
            } => {
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&format_position_question(
                    *test_letter,
                    test_position,
                    *requirement_letter,
                    requirement_position,
                ));
                out.push('\n');

                let child_prefix = format!("{prefix}   ");
                render_no_branch(no, &format!("{child_prefix}│"), out);
                render_yes_final(yes, &child_prefix, out);
            }
            Node::YesSplit { test_letter, test_position, requirement_letter, requirement_position, yes } => {
                // YesSplit in a No branch position
                out.push_str(prefix);
                out.push_str("└─ No: ");
                out.push_str(&format_position_question(
                    *test_letter,
                    test_position,
                    *requirement_letter,
                    requirement_position,
                ));
                out.push_str(" (yes only)\n");

                let child_prefix = format!("{prefix}   ");
                // No "no" branch to render for YesSplit
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

                render_no_branch(no, &format!("{prefix}│"), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_yes_final(&Node::Leaf(word.clone()), prefix, out);
            }
            Node::PositionalSplit {
                test_letter,
                test_position,
                requirement_letter,
                requirement_position,
                yes,
                no,
            } => {
                // For a positional split in the Yes position, continue the spine pattern
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str(&format_position_question(
                    *test_letter,
                    test_position,
                    *requirement_letter,
                    requirement_position,
                ));
                out.push('\n');

                render_no_branch(no, &format!("{prefix}│"), out);

                // No spacer line needed - next node will add its own if needed

                render_yes_final(yes, prefix, out);
            }
            Node::YesSplit { test_letter, test_position, requirement_letter, requirement_position, yes } => {
                // YesSplit: like a hard split but with no "no" branch
                out.push_str(prefix);
                out.push_str("│\n");

                out.push_str(prefix);
                out.push_str(&format_position_question(
                    *test_letter,
                    test_position,
                    *requirement_letter,
                    requirement_position,
                ));
                out.push_str(" (yes only)\n");

                // No "no" branch to render
                // No spacer line needed - next node will add its own if needed

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

                render_no_branch(no, &format!("{prefix}│"), out);

                out.push_str(prefix);
                out.push_str("│\n");

                render_spine(&Node::Leaf(word.clone()), prefix, is_final, out);
            }
            Node::PositionalSplit {
                test_letter,
                test_position,
                requirement_letter,
                requirement_position,
                yes,
                no,
            } => {
                // Print the question
                out.push_str(prefix);
                out.push_str(&format_position_question(
                    *test_letter,
                    test_position,
                    *requirement_letter,
                    requirement_position,
                ));
                out.push('\n');

                // No branch diverges sideways
                render_no_branch(no, &format!("{prefix}│"), out);

                // Spacer line for clarity between decision points
                out.push_str(prefix);
                out.push_str("│\n");

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
            Node::YesSplit { test_letter, test_position, requirement_letter, requirement_position, yes } => {
                // YesSplit: like a hard split but with no "no" branch
                out.push_str(prefix);
                out.push_str(&format_position_question(
                    *test_letter,
                    test_position,
                    *requirement_letter,
                    requirement_position,
                ));
                out.push_str(" (yes only)\n");

                // No "no" branch to render
                // No spacer line needed - next node will add its own if needed

                // Continue down the Yes spine
                render_spine(yes, prefix, is_final, out);
            }
        }
    }

    let mut out = String::new();
    render_spine(node, "", true, &mut out);
    out
}
