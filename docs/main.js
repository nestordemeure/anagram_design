import init, { solve_words, zodiac_words } from "./pkg/anagram_design.js";

const wordsField = document.querySelector("#words");
const allowRepeatField = document.querySelector("#allow-repeat");
const prioritizeSoftField = document.querySelector("#prioritize-soft");
const limitField = document.querySelector("#limit");
const statusEl = document.querySelector("#status");
const summaryEl = document.querySelector("#summary");
const treesEl = document.querySelector("#trees");
const formEl = document.querySelector("#solver-form");

const wasmReady = init();

// LocalStorage key for storing user choices
const STORAGE_KEY = "anagram_tree_choices";

// Auto-resize textarea based on content
function autoResizeTextarea() {
  wordsField.style.height = 'auto';
  wordsField.style.height = wordsField.scrollHeight + 'px';
}

// Set up auto-resize on input
wordsField.addEventListener('input', autoResizeTextarea);
// Initial resize
autoResizeTextarea();

function setStatus(message, tone = "neutral") {
  statusEl.textContent = message;
  statusEl.dataset.tone = tone;
}

function escapeHtml(str) {
  return str
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function parseWords() {
  return wordsField.value
    .split(/[\n,]+/)
    .map((w) => w.trim())
    .filter(Boolean);
}

function setWords(words) {
  wordsField.value = words.join("\n");
  autoResizeTextarea();
}

// Get user's selected option index for a given tree path (default to 0)
function getSelectedOption(path) {
  try {
    const choices = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return choices[path] ?? 0;
  } catch {
    return 0;
  }
}

// Save user's selection for a tree path
function saveSelection(path, optionIndex) {
  try {
    const choices = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    choices[path] = optionIndex;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(choices));
  } catch (err) {
    console.error("Failed to save selection:", err);
  }
}

// Capitalize first letter
function capitalizeFirst(word) {
  if (!word) return word;
  return word[0].toUpperCase() + word.slice(1);
}

// Display letter in uppercase
function displayLetter(c) {
  return c.toUpperCase();
}

// Describe position for mirror splits
function describePos(fromEnd, idx) {
  const positions = {
    false: { 1: "first", 2: "second", 3: "third" },
    true: { 1: "last", 2: "second-to-last", 3: "third-to-last" }
  };
  return positions[fromEnd]?.[idx] ?? `pos ${idx}`;
}

// Format position name for display
function formatPosition(position) {
  const positions = {
    "Contains": "Contains",
    "First": "First letter",
    "Second": "Second letter",
    "Third": "Third letter",
    "ThirdToLast": "Third-to-last letter",
    "SecondToLast": "Second-to-last letter",
    "Last": "Last letter",
    "Double": "Double",
    "Triple": "Triple"
  };
  return positions[position] || position;
}

// Format requirement description for soft splits
function formatRequirement(position, letter) {
  const requirements = {
    "Contains": `all No contain '${displayLetter(letter)}'`,
    "First": `all No have '${displayLetter(letter)}' first`,
    "Second": `all No have '${displayLetter(letter)}' second`,
    "Third": `all No have '${displayLetter(letter)}' third`,
    "ThirdToLast": `all No have '${displayLetter(letter)}' third-to-last`,
    "SecondToLast": `all No have '${displayLetter(letter)}' second-to-last`,
    "Last": `all No have '${displayLetter(letter)}' last`,
    "Double": `all No double '${displayLetter(letter)}'`,
    "Triple": `all No triple '${displayLetter(letter)}'`
  };
  return requirements[position] || `requirement ${position}`;
}

// Format node info as a question string (similar to Rust's format.rs)
function formatNodeInfo(info) {
  if (!info) {
    console.error("formatNodeInfo called with undefined info!");
    return "[Error: undefined info]";
  }

  switch (info.type) {
    case "leaf":
      return capitalizeFirst(info.word);
    case "repeat":
      return `Repeat ${capitalizeFirst(info.word)}, ${capitalizeFirst(info.word)}, ${capitalizeFirst(info.word)}...`;
    case "positionalSplit": {
      const { testLetter, testPosition, requirementLetter, requirementPosition } = info;

      // Hard split: test and requirement are the same
      if (testLetter === requirementLetter && testPosition === requirementPosition) {
        return `${formatPosition(testPosition)} '${displayLetter(testLetter)}'?`;
      } else {
        // Soft split: different test and requirement
        const testDesc = `${formatPosition(testPosition)} '${displayLetter(testLetter)}'?`;
        const reqDesc = formatRequirement(requirementPosition, requirementLetter);
        return `${testDesc} (${reqDesc})`;
      }
    }
    default:
      console.error("Unknown node type:", info.type);
      return "[Unknown node type]";
  }
}

// Check if node is a leaf (no children)
function isLeaf(option) {
  return option.info.type === "leaf";
}

// Render a No branch that diverges sideways
function renderNoBranch(mergedNode, path, prefix, out) {
  const selectedIdx = Math.min(getSelectedOption(path), mergedNode.options.length - 1);
  const option = mergedNode.options[selectedIdx];
  const isChoice = mergedNode.options.length > 1;

  const nodeText = formatNodeInfo(option.info);
  const isLeafNode = isLeaf(option);

  if (isLeafNode) {
    out.lines.push(`${prefix}└─ No: ${nodeText}`);
  } else if (option.info.type === "repeat") {
    out.lines.push(`${prefix}└─ No: ${nodeText}`);
    const childPrefix = `${prefix}   `;
    if (option.noBranch) {
      renderNoBranch(option.noBranch, `${path}_no`, `${childPrefix}│`, out);
    }
    renderYesFinal({ options: [{ info: { type: "leaf", word: option.info.word } }] }, `${path}_yes`, childPrefix, out);
  } else {
    // Another split in the No branch
    const marker = isChoice ? `<span class="choice-node" data-path="${path}">└─ No: ${nodeText} ▼</span>` : `└─ No: ${nodeText}`;
    out.lines.push(`${prefix}${marker}`);

    if (isChoice) {
      out.choices.push({ path, options: mergedNode.options, selectedIdx });
    }

    const childPrefix = `${prefix}   `;
    if (option.noBranch) {
      renderNoBranch(option.noBranch, `${path}_no`, `${childPrefix}│`, out);
    }
    if (option.yesBranch) {
      renderYesFinal(option.yesBranch, `${path}_yes`, childPrefix, out);
    }
  }
}

// Render a final Yes item
function renderYesFinal(mergedNode, path, prefix, out) {
  const selectedIdx = Math.min(getSelectedOption(path), mergedNode.options.length - 1);
  const option = mergedNode.options[selectedIdx];
  const isChoice = mergedNode.options.length > 1;

  const nodeText = formatNodeInfo(option.info);
  const isLeafNode = isLeaf(option);

  if (isLeafNode) {
    out.lines.push(`${prefix}└─ ${nodeText}`);
  } else if (option.info.type === "repeat") {
    out.lines.push(`${prefix}│`);
    const marker = isChoice ? `<span class="choice-node" data-path="${path}">${nodeText} ▼</span>` : nodeText;
    out.lines.push(`${prefix}${marker}`);

    if (isChoice) {
      out.choices.push({ path, options: mergedNode.options, selectedIdx });
    }

    if (option.noBranch) {
      renderNoBranch(option.noBranch, `${path}_no`, `${prefix}│`, out);
    }
    out.lines.push(`${prefix}│`);
    renderYesFinal({ options: [{ info: { type: "leaf", word: option.info.word } }] }, `${path}_yes`, prefix, out);
  } else {
    // Split in Yes position - continue the spine
    out.lines.push(`${prefix}│`);
    const marker = isChoice ? `<span class="choice-node" data-path="${path}">${nodeText} ▼</span>` : nodeText;
    out.lines.push(`${prefix}${marker}`);

    if (isChoice) {
      out.choices.push({ path, options: mergedNode.options, selectedIdx });
    }

    if (option.noBranch) {
      renderNoBranch(option.noBranch, `${path}_no`, `${prefix}│`, out);
    }
    out.lines.push(`${prefix}│`);
    if (option.yesBranch) {
      renderYesFinal(option.yesBranch, `${path}_yes`, prefix, out);
    }
  }
}

// Render the main Yes spine
function renderSpine(mergedNode, path, prefix, isFinal, out) {
  if (!mergedNode || !mergedNode.options || mergedNode.options.length === 0) {
    return;
  }

  const selectedIdx = Math.min(getSelectedOption(path), mergedNode.options.length - 1);
  const option = mergedNode.options[selectedIdx];
  const isChoice = mergedNode.options.length > 1;

  const nodeText = formatNodeInfo(option.info);
  const isLeafNode = isLeaf(option);

  if (isLeafNode) {
    const connector = isFinal ? "└─ " : "├─ ";
    out.lines.push(`${prefix}${connector}${nodeText}`);
  } else if (option.info.type === "repeat") {
    const marker = isChoice ? `<span class="choice-node" data-path="${path}">${nodeText} ▼</span>` : nodeText;
    out.lines.push(`${prefix}${marker}`);

    if (isChoice) {
      out.choices.push({ path, options: mergedNode.options, selectedIdx });
    }

    if (option.noBranch) {
      renderNoBranch(option.noBranch, `${path}_no`, `${prefix}│`, out);
    }
    out.lines.push(`${prefix}│`);
    renderSpine({ options: [{ info: { type: "leaf", word: option.info.word } }] }, `${path}_yes`, prefix, isFinal, out);
  } else {
    // Regular split - print question
    const marker = isChoice ? `<span class="choice-node" data-path="${path}">${nodeText} ▼</span>` : nodeText;
    out.lines.push(`${prefix}${marker}`);

    if (isChoice) {
      out.choices.push({ path, options: mergedNode.options, selectedIdx });
    }

    // No branch diverges sideways
    if (option.noBranch) {
      renderNoBranch(option.noBranch, `${path}_no`, `${prefix}│`, out);
    }

    // Spacer line
    out.lines.push(`${prefix}│`);

    // Continue down Yes spine
    if (option.yesBranch) {
      renderSpine(option.yesBranch, `${path}_yes`, prefix, isFinal, out);
    }
  }
}

// Render the merged tree to HTML
function renderMergedTree(mergedTree) {
  const out = { lines: [], choices: [] };
  renderSpine(mergedTree, "root", "", true, out);

  const htmlLines = out.lines.map(line => escapeHtml(line)).join("\n");
  // Un-escape our choice nodes (they contain safe HTML)
  const withChoices = htmlLines.replace(/&lt;span class="choice-node"(.*?)&gt;(.*?)&lt;\/span&gt;/g,
    '<span class="choice-node"$1>$2</span>');

  return { html: withChoices, choices: out.choices };
}

// Create dropdown menu for a choice node
function createDropdown(options, path, currentIdx) {
  const dropdown = document.createElement("div");
  dropdown.className = "choice-dropdown";
  dropdown.style.cssText = `
    position: absolute;
    background: var(--pre-bg, #111827);
    border: 2px solid var(--choice-color, #60a5fa);
    border-radius: 4px;
    padding: 0;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    z-index: 1000;
    max-height: 400px;
    overflow-y: auto;
    font-family: "JetBrains Mono", "Fira Code", ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
    font-size: inherit;
    color: var(--pre-fg, #e5e7eb);
  `;

  options.forEach((opt, idx) => {
    const item = document.createElement("div");
    item.className = "dropdown-item";
    const isSelected = idx === currentIdx;
    item.style.cssText = `
      padding: 0.25rem 0.75rem;
      cursor: pointer;
      white-space: nowrap;
      ${isSelected ? 'background: rgba(96, 165, 250, 0.2); border-left: 3px solid var(--choice-color, #60a5fa);' : 'border-left: 3px solid transparent;'}
    `;
    item.textContent = formatNodeInfo(opt.info);

    item.addEventListener("mouseenter", () => {
      if (!isSelected) {
        item.style.background = "rgba(96, 165, 250, 0.1)";
      }
    });
    item.addEventListener("mouseleave", () => {
      if (!isSelected) {
        item.style.background = "";
      }
    });
    item.addEventListener("click", () => {
      saveSelection(path, idx);
      // Re-render the entire tree
      const currentResult = window.currentResult;
      if (currentResult) {
        renderResult(currentResult);
      }
      dropdown.remove();
    });

    dropdown.appendChild(item);
  });

  return dropdown;
}

// Attach click handlers to choice nodes
function attachChoiceHandlers(choices) {
  choices.forEach(({ path, options, selectedIdx }) => {
    const nodes = document.querySelectorAll(`span.choice-node[data-path="${path}"]`);
    nodes.forEach(node => {
      node.style.cursor = "pointer";
      node.style.fontWeight = "bold";

      node.addEventListener("click", (e) => {
        e.stopPropagation();

        // Remove any existing dropdowns
        document.querySelectorAll(".choice-dropdown").forEach(d => d.remove());

        // Create and position dropdown
        const dropdown = createDropdown(options, path, selectedIdx);
        const rect = node.getBoundingClientRect();

        // Position dropdown aligned with the clicked node
        // Use absolute positioning with scroll offset so it moves with the content
        dropdown.style.left = `${rect.left + window.scrollX}px`;
        dropdown.style.top = `${rect.top + window.scrollY}px`;

        document.body.appendChild(dropdown);

        // Close dropdown when clicking outside
        const closeHandler = (ev) => {
          if (!dropdown.contains(ev.target)) {
            dropdown.remove();
            document.removeEventListener("click", closeHandler);
          }
        };

        setTimeout(() => document.addEventListener("click", closeHandler), 0);
      });
    });
  });
}

function renderResult(result) {
  window.currentResult = result; // Store for re-rendering

  const { cost, merged_tree: mergedTree, exhausted } = result;

  const { html, choices } = renderMergedTree(mergedTree);

  const hasChoices = choices.length > 0;
  const instruction = hasChoices
    ? `<p><em>Click on <span class="choice-hint">blue nodes ▼</span> to pick alternative options.</em></p>`
    : "";

  treesEl.innerHTML = `
    ${instruction}
    <article>
      <pre>${html}</pre>
    </article>
  `;

  if (exhausted) {
    treesEl.insertAdjacentHTML(
      "beforeend",
      `<p><small>Results were truncated; more optimal trees exist beyond the requested limit.</small></p>`
    );
  }

  // Cost summary goes below the tree
  const summaryText = `
    Maximum number of "No" answers: ${cost.max_nos} (average: ${cost.avg_nos.toFixed(2)}) ·
    Maximum number of unjustified "No" answers: ${cost.max_hard_nos} (average: ${cost.avg_hard_nos.toFixed(2)})
  `;
  summaryEl.innerHTML = `<p>${summaryText}</p>`;

  // Attach click handlers to choice nodes
  attachChoiceHandlers(choices);
}

async function runSolver(event) {
  event?.preventDefault();
  await wasmReady;

  const words = parseWords();
  const limit = Number.parseInt(limitField.value, 10);
  const normalizedLimit = Number.isFinite(limit) ? Math.max(0, limit) : 5;

  try {
    setStatus("Generating…");
    const result = solve_words(
      words,
      allowRepeatField.checked,
      prioritizeSoftField.checked,
      normalizedLimit
    );
    renderResult(result);
    setStatus("");
  } catch (err) {
    console.error(err);
    const message = err instanceof Error ? err.message : String(err);
    setStatus(message, "error");
  }
}

async function initDefaults() {
  await wasmReady;
  const defaults = zodiac_words();
  if (Array.isArray(defaults)) {
    setWords(defaults.map(capitalizeFirst));
  }
  runSolver();
}

formEl.addEventListener("submit", runSolver);

initDefaults().catch((err) => {
  console.error(err);
  setStatus("Failed to load WebAssembly module.");
});
