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
}

function renderResult(result) {
  const { cost, trees, exhausted } = result;
  const summaryText = `
    Maximum number of “No” answers: ${cost.max_nos} (average: ${cost.avg_nos.toFixed(2)}) ·
    Maximum number of unjustified “No” answers: ${cost.max_hard_nos} (average: ${cost.avg_hard_nos.toFixed(2)})
  `;
  summaryEl.innerHTML = `<p>${summaryText}</p>`;

  const treeBodies = trees
    .map(
      (tree, idx) => `
        <h4>Tree ${idx + 1}</h4>
        <pre>${escapeHtml(tree)}</pre>`
    )
    .join("");

  treesEl.innerHTML = `<article>${treeBodies}</article>`;

  if (exhausted) {
    treesEl.insertAdjacentHTML(
      "beforeend",
      `<p><small>Results were truncated; more optimal trees exist beyond the requested limit.</small></p>`
    );
  }
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

function capitalizeFirst(word) {
  if (!word) return word;
  return word[0].toUpperCase() + word.slice(1);
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
