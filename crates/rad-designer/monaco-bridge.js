// Drives Monaco editor instances living in the *parent* page (index.html)
// from this module's own realm (the #preview iframe's wasm-bindgen glue
// imports this file directly). `window`/`document` here are the iframe's
// own — every reference to the parent page's globals goes through
// `window.parent` explicitly, the same rule the Rust side follows for
// `chrome_element`.
//
// Monaco itself is loaded by index.html via its classic AMD loader
// (script tag + `require`), not as a bundled dependency: that's the
// simplest way to get the real VS Code editor into a trunk-served,
// bundler-less static page. `window.parent.radMonacoReady` is a Promise
// index.html resolves once `vs/editor/editor.main` has loaded.

let panels = null; // the parent-page element new editor panes mount into
let editors = {}; // path -> { pane, editor, model }
let onChange = null;

export function init(panelsEl) {
  panels = panelsEl;
}

export function setOnChange(cb) {
  onChange = cb;
}

function languageFor(path) {
  return path.endsWith(".rs") ? "rust" : "plaintext";
}

export async function openFile(path, content, readOnly) {
  await window.parent.radMonacoReady;
  const monaco = window.parent.monaco;
  const existing = editors[path];
  if (existing) {
    if (existing.model.getValue() !== content) {
      existing.model.setValue(content);
    }
    // The caller (`open_file_tab`) already called `switch_tab`, which
    // calls `show` synchronously right after this — but on a path's
    // *first* open, below, that `show` call lands before the pane
    // exists (this function hadn't gotten past the `await` above yet),
    // so nothing was there to show. Calling `show` again here, once
    // we're actually ready, is what makes the tab appear without the
    // user having to click away and back.
    show(path);
    return;
  }
  const doc = window.parent.document;
  const pane = doc.createElement("div");
  pane.className = "monaco-pane";
  pane.style.cssText = "display:none; width:100%; height:100%;";
  panels.appendChild(pane);

  const model = monaco.editor.createModel(content, languageFor(path));
  const editor = monaco.editor.create(pane, {
    model,
    readOnly: !!readOnly,
    automaticLayout: true,
    theme: "vs",
    minimap: { enabled: false },
    fontSize: 13,
  });
  model.onDidChangeContent(() => {
    if (onChange) onChange(path, model.getValue());
  });
  editors[path] = { pane, editor, model };
  show(path);
}

export function show(path) {
  for (const p in editors) {
    editors[p].pane.style.display = p === path ? "block" : "none";
  }
}

export function setContent(path, content) {
  const existing = editors[path];
  if (existing && existing.model.getValue() !== content) {
    existing.model.setValue(content);
  }
}

export function getContent(path) {
  const existing = editors[path];
  return existing ? existing.model.getValue() : null;
}

export function revealLine(path, line) {
  const existing = editors[path];
  if (!existing) return;
  existing.editor.revealLineInCenter(line);
  existing.editor.setPosition({ lineNumber: line, column: 1 });
  existing.editor.focus();
}

export function closeFile(path) {
  const existing = editors[path];
  if (!existing) return;
  existing.editor.dispose();
  existing.model.dispose();
  existing.pane.remove();
  delete editors[path];
}
