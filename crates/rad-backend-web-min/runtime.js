// Hand-written counterpart to `src/lib.rs` — the `env` import object a
// compiled app needs, since there's no wasm-bindgen glue to generate it.
// One instance of this per running app; `instantiate` below wires it up
// and also exposes `rad_dispatch_click`/`rad_dispatch_resize`, which this
// file calls back into on real DOM events.
//
// Element ids are plain array indices into `elements`; window ids share
// the same table (a window is just an element whose "pos/size" is really
// its outer size, and whose "text" is really its title bar).

const KIND_BUTTON = 0;
const KIND_LABEL = 1;
const KIND_CHECKBOX = 2;
const KIND_TEXTBOX = 3;
const KIND_RADIO_BUTTON = 4;
const KIND_PROGRESS_BAR = 5;
const KIND_PICTURE_BOX = 6;
const KIND_TREE_VIEW = 7;

function readString(memory, ptr, len) {
  return new TextDecoder("utf-8").decode(new Uint8Array(memory.buffer, ptr, len));
}

/**
 * Compiles and instantiates a compiled app's `.wasm` into `root`
 * (a DOM element the app's window(s) get mounted into).
 */
export async function runApp(wasmBytes, root) {
  const elements = []; // { kind, dom, clickCallbackId, resizeCallbackId }
  let memory; // set once the instance exists; imports below close over it
  let windowCount = 0;

  function newElement(kind, dom) {
    const id = elements.length;
    elements.push({ kind, dom, clickCallbackId: null, resizeCallbackId: null });
    return id;
  }

  // The app's first window embeds directly into the host page — that
  // page IS its "desktop", same as the designer's own preview. Every
  // window after that is a real, separate OS-level browser window
  // (`Form2.Show()`-style), not another div layered into the same page:
  // a second chrome drawn inside the first would have no way to be
  // dragged, minimized, or alt-tabbed to independently, and a real
  // `window.open()` gets all of that for free.
  function createEmbeddedWindow(title, width, height) {
    const chrome = document.createElement("div");
    chrome.className = "rad-window";
    chrome.style.cssText = `position:relative; display:flex; flex-direction:column; width:${width}px; height:${height}px; border:1px solid #888; overflow:hidden;`;

    const titleBar = document.createElement("div");
    titleBar.className = "rad-window-titlebar";
    titleBar.textContent = title;
    titleBar.style.cssText = "flex:0 0 auto; background:#2b579a; color:#fff; padding:6px 8px; font:12px sans-serif;";
    chrome.appendChild(titleBar);

    const body = document.createElement("div");
    body.className = "rad-window-body";
    body.style.cssText = "position:relative; flex:1 1 auto;";
    chrome.appendChild(body);

    root.appendChild(chrome);

    const id = newElement("window", chrome);
    elements[id].titleBar = titleBar;
    elements[id].body = body;
    elements[id].realWindow = null;

    new ResizeObserver((entries) => {
      const rect = entries[0].contentRect;
      const el = elements[id];
      if (el.resizeCallbackId !== null) {
        instance.exports.rad_dispatch_resize(el.resizeCallbackId, Math.round(rect.width), Math.round(rect.height));
      }
    }).observe(chrome);

    return id;
  }

  // The real browser window already draws its own title bar and frame,
  // so this skips the hand-drawn chrome entirely — its `body` is the
  // whole window, not a child of some larger chrome element.
  function createPoppedWindow(popped, title, width, height) {
    const doc = popped.document;
    doc.title = title;
    doc.body.style.cssText = "margin:0; font:12px sans-serif; position:relative; overflow:hidden;";

    const id = newElement("window", doc.body);
    elements[id].titleBar = null;
    elements[id].body = doc.body;
    elements[id].realWindow = popped;
    elements[id].doc = doc;

    popped.addEventListener("resize", () => {
      const el = elements[id];
      if (el.resizeCallbackId !== null) {
        instance.exports.rad_dispatch_resize(el.resizeCallbackId, doc.body.clientWidth, doc.body.clientHeight);
      }
    });

    return id;
  }

  const env = {
    rad_create_window(titlePtr, titleLen, width, height) {
      const title = readString(memory, titlePtr, titleLen);
      windowCount += 1;
      if (windowCount === 1) {
        return createEmbeddedWindow(title, width, height);
      }
      const popped = window.open("", "_blank", `width=${width},height=${height}`);
      // A popup blocked by the browser (most likely: this call didn't
      // happen inside a direct user-gesture handler) shouldn't just lose
      // the window — fall back to embedding it like the first one.
      return popped ? createPoppedWindow(popped, title, width, height) : createEmbeddedWindow(title, width, height);
    },

    rad_create_element(parentId, kind, textPtr, textLen, groupPtr, groupLen) {
      const text = readString(memory, textPtr, textLen);
      const group = groupLen > 0 ? readString(memory, groupPtr, groupLen) : "";
      const parent = elements[parentId].body;

      let dom;
      switch (kind) {
        case KIND_BUTTON:
          dom = document.createElement("button");
          dom.textContent = text;
          break;
        case KIND_LABEL:
          dom = document.createElement("span");
          dom.textContent = text;
          break;
        case KIND_CHECKBOX:
        case KIND_RADIO_BUTTON: {
          dom = document.createElement("label");
          const input = document.createElement("input");
          input.type = kind === KIND_CHECKBOX ? "checkbox" : "radio";
          if (group) input.name = group;
          dom.appendChild(input);
          const span = document.createElement("span");
          span.textContent = text;
          dom.appendChild(span);
          dom._radInput = input;
          dom._radTextEl = span;
          break;
        }
        case KIND_TEXTBOX:
          dom = document.createElement("input");
          dom.type = "text";
          dom.value = text;
          break;
        case KIND_PROGRESS_BAR:
          dom = document.createElement("progress");
          dom.max = 100;
          dom.value = 0;
          break;
        case KIND_PICTURE_BOX:
          dom = document.createElement("img");
          dom.style.objectFit = "contain";
          if (text) dom.src = text;
          break;
        case KIND_TREE_VIEW:
          dom = document.createElement("div");
          dom.style.cssText += "overflow:auto; border:1px solid #adadad; background:#fff;";
          break;
        default:
          dom = document.createElement("div");
      }
      dom.style.position = "absolute";
      parent.appendChild(dom);
      return newElement(kind, dom);
    },

    rad_set_pos(id, x, y) {
      const dom = elements[id].dom;
      dom.style.left = `${x}px`;
      dom.style.top = `${y}px`;
    },
    rad_get_pos_x(id) { return elements[id].dom.offsetLeft; },
    rad_get_pos_y(id) { return elements[id].dom.offsetTop; },
    // A window's "width"/"height" is really the real browser window's
    // outer size once it's popped out — not its body div's CSS width.
    rad_set_width(id, width) {
      const el = elements[id];
      if (el.realWindow) el.realWindow.resizeTo(width, el.realWindow.outerHeight);
      else el.dom.style.width = `${width}px`;
    },
    rad_get_width(id) {
      const el = elements[id];
      return el.realWindow ? el.realWindow.innerWidth : el.dom.offsetWidth;
    },
    rad_set_height(id, height) {
      const el = elements[id];
      if (el.realWindow) el.realWindow.resizeTo(el.realWindow.outerWidth, height);
      else el.dom.style.height = `${height}px`;
    },
    rad_get_height(id) {
      const el = elements[id];
      return el.realWindow ? el.realWindow.innerHeight : el.dom.offsetHeight;
    },

    rad_set_text(id, ptr, len) {
      const text = readString(memory, ptr, len);
      const el = elements[id];
      if (el.kind === "window") {
        if (el.titleBar) el.titleBar.textContent = text;
        (el.doc ?? document).title = text;
      } else if (el.dom._radTextEl) {
        el.dom._radTextEl.textContent = text;
      } else if (el.dom.tagName === "INPUT") {
        el.dom.value = text;
      } else if (el.dom.tagName === "IMG") {
        el.dom.src = text;
      } else {
        el.dom.textContent = text;
      }
    },
    rad_get_text_len(id) {
      elements[id]._pendingText = textOf(elements[id]);
      return new TextEncoder().encode(elements[id]._pendingText).length;
    },
    rad_get_text_into(id, ptr) {
      const bytes = new TextEncoder().encode(elements[id]._pendingText ?? "");
      new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    },

    rad_set_checked(id, checked) {
      elements[id].dom._radInput.checked = !!checked;
    },
    rad_get_checked(id) {
      return elements[id].dom._radInput.checked ? 1 : 0;
    },

    rad_register_click(id, callbackId) {
      const el = elements[id];
      el.clickCallbackId = callbackId;
      el.dom.addEventListener("click", () => {
        instance.exports.rad_dispatch_click(callbackId);
      });
    },
    rad_register_resize(id, callbackId) {
      // Actual dispatch happens from the ResizeObserver set up in
      // rad_create_window; this just remembers which callback to use.
      elements[id].resizeCallbackId = callbackId;
    },
    rad_set_default_icon(id) {
      // No icon story for a page embedded in someone else's site; a
      // standalone download sets its own favicon in the host HTML
      // instead. Intentionally a no-op here.
    },

    rad_set_value(id, value) {
      elements[id].dom.value = value;
    },
    rad_get_value(id) {
      return elements[id].dom.value;
    },

    // One row per line, `"<depth>\t<text>"` — matches how the Rust
    // side encodes it; simplest way to send a whole list across in one
    // FFI call. Indented flat rows, not a real nested `<ul>`, for the
    // same reason `rad-backend-web`'s own tree view renders this way.
    rad_set_tree_items(id, ptr, len) {
      const text = readString(memory, ptr, len);
      const dom = elements[id].dom;
      dom.textContent = "";
      if (!text) return;
      for (const line of text.split("\n")) {
        const tabIndex = line.indexOf("\t");
        const depth = Number(line.slice(0, tabIndex)) || 0;
        const label = line.slice(tabIndex + 1);
        const row = document.createElement("div");
        row.textContent = label;
        row.style.paddingLeft = `${depth * 16}px`;
        row.style.whiteSpace = "nowrap";
        dom.appendChild(row);
      }
    },

    rad_set_fore_color(id, hasColor, r, g, b) {
      // Clearing the property (not setting `"inherit"`) is what lets a
      // control show its parent's own text color through, same as back
      // color's `hasColor` handling below.
      elements[id].dom.style.color = hasColor ? `rgb(${r},${g},${b})` : "";
    },
    rad_set_back_color(id, hasColor, r, g, b) {
      // Clearing the property (not setting `"transparent"`) is what
      // lets a control show its parent's own background through — the
      // same default every control here starts with.
      elements[id].dom.style.backgroundColor = hasColor ? `rgb(${r},${g},${b})` : "";
    },
  };

  function textOf(el) {
    if (el.kind === "window") return el.titleBar ? el.titleBar.textContent : (el.doc ?? document).title;
    if (el.dom._radTextEl) return el.dom._radTextEl.textContent;
    if (el.dom.tagName === "INPUT") return el.dom.value;
    if (el.dom.tagName === "IMG") return el.dom.getAttribute("src") ?? "";
    return el.dom.textContent;
  }

  const { instance } = await WebAssembly.instantiate(wasmBytes, { env });
  memory = instance.exports.memory;
  if (instance.exports.main) {
    instance.exports.main();
  }
  return instance;
}
