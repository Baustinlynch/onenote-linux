//! A navigation bar for the main window.
//!
//! Tauri has no history API, and a second webview would need the `unstable`
//! feature plus a migration away from `get_webview_window`. Instead the bar is
//! injected into every page the window loads and drives `window.history`
//! directly, so it survives full navigations and works with OneNote's own
//! client-side routing.
//!
//! It lives in a shadow root: OneNote is a large app with aggressive global
//! CSS, and a plain element would be restyled or hidden by it.

/// Injected into the main webview on every page load.
pub const SCRIPT: &str = r#"
(function () {
  if (window.__onenoteBarInstalled) { return; }
  window.__onenoteBarInstalled = true;

  // OneNote replaces history entries from its router, so track a stack rather
  // than trusting history.length to tell us which way we can move.
  var stack = [0];
  var pos = 0;
  var atTop = function () { return pos <= 0; };
  var atEnd = function () { return pos >= stack.length - 1; };

  var HOST_ID = "__onenote_nav_host";
  var collapsed = false;

  function icon(path) {
    return '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">' + path + "</svg>";
  }
  var BACK = icon('<path fill="currentColor" d="M15.5 4 7 12l8.5 8V4z"/>');
  var FORWARD = icon('<path fill="currentColor" d="M8.5 4 17 12l-8.5 8V4z"/>');
  var RELOAD = icon('<path fill="currentColor" d="M12 5V2L7 6l5 4V7a5 5 0 1 1-5 5H5a7 7 0 1 0 7-7z"/>');
  var HIDE = icon('<path fill="currentColor" d="M19 13H5v-2h14v2z"/>');
  var SHOW = icon('<path fill="currentColor" d="M4 5h16v2H4V5zm0 6h16v2H4v-2zm0 6h10v2H4v-2z"/>');

  function build() {
    var host = document.createElement("div");
    host.id = HOST_ID;
    // Fixed, topmost, and transparent to layout so OneNote's own header keeps
    // its position underneath.
    host.style.cssText = "position:fixed;top:0;left:0;z-index:2147483647;";

    var root = host.attachShadow({ mode: "open" });
    var style = document.createElement("style");
    style.textContent = [
      ":host{all:initial}",
      ".bar{display:flex;align-items:center;gap:2px;padding:2px 3px;",
      "font:13px/1 -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;",
      "background:rgba(32,32,32,.86);border-radius:0 0 8px 0;",
      "backdrop-filter:blur(8px);-webkit-backdrop-filter:blur(8px);",
      "box-shadow:0 1px 6px rgba(0,0,0,.35);color:#f2f2f2;",
      "-webkit-app-region:no-drag}",
      "button{all:unset;display:flex;align-items:center;justify-content:center;",
      "width:26px;height:24px;border-radius:4px;cursor:pointer;color:inherit}",
      "button:hover:not(:disabled){background:rgba(255,255,255,.16)}",
      "button:active:not(:disabled){background:rgba(255,255,255,.28)}",
      "button:disabled{opacity:.32;cursor:default}",
      "button:focus-visible{outline:2px solid #6ea8fe;outline-offset:-2px}",
      ".sep{width:1px;height:16px;background:rgba(255,255,255,.22);margin:0 2px}",
      // Collapsed state leaves only the reveal button visible.
      ".hide{display:none}",
      ".pill{position:fixed;top:0;left:0;display:flex}",
    ].join("");
    root.appendChild(style);

    var pill = document.createElement("div");
    pill.className = "pill";

    function button(html, title) {
      var b = document.createElement("button");
      b.type = "button";
      b.title = title;
      b.setAttribute("aria-label", title);
      b.innerHTML = html;
      pill.appendChild(b);
      return b;
    }

    var back = button(BACK, "Back (Alt+Left)");
    var fwd = button(FORWARD, "Forward (Alt+Right)");
    var sep = document.createElement("div");
    sep.className = "sep";
    pill.appendChild(sep);
    var reload = button(RELOAD, "Reload");
    var collapse = button(HIDE, "Hide navigation bar");

    back.addEventListener("click", function () { history.back(); });
    fwd.addEventListener("click", function () { history.forward(); });
    reload.addEventListener("click", function () { location.reload(); });
    collapse.addEventListener("click", function () {
      collapsed = true;
      back.classList.add("hide");
      fwd.classList.add("hide");
      sep.classList.add("hide");
      reload.classList.add("hide");
      collapse.innerHTML = SHOW;
      collapse.title = "Show navigation bar";
    });

    // Clicking the bar again brings it back.
    pill.addEventListener("click", function (event) {
      if (!collapsed) { return; }
      if (event.target.closest("button") !== collapse) { return; }
      collapsed = false;
      back.classList.remove("hide");
      fwd.classList.remove("hide");
      sep.classList.remove("hide");
      reload.classList.remove("hide");
      collapse.innerHTML = HIDE;
      collapse.title = "Hide navigation bar";
      sync();
    });

    root.appendChild(pill);
    return { host: host, back: back, fwd: fwd };
  }

  var ui = null;

  function sync() {
    if (!ui) { return; }
    ui.back.disabled = atTop();
    ui.fwd.disabled = atEnd();
  }

  // OneNote's router pushes entries after load, so keep the buttons current
  // whenever history changes rather than only on user input.
  ["pushState", "replaceState"].forEach(function (name) {
    var original = history[name];
    if (typeof original !== "function") { return; }
    history[name] = function () {
      if (name === "pushState") {
        stack = stack.slice(0, pos + 1);
        stack.push(++pos);
      }
      var result = original.apply(this, arguments);
      sync();
      return result;
    };
  });

  window.addEventListener("popstate", function () {
    // A pop moves one entry; a full document load is handled by a fresh
    // script run with an empty stack.
    if (pos > 0) { pos -= 1; }
    sync();
  });

  window.addEventListener("keydown", function (event) {
    if (!event.altKey || event.ctrlKey || event.metaKey) { return; }
    if (event.key === "ArrowLeft") { history.back(); }
    else if (event.key === "ArrowRight") { history.forward(); }
    else { return; }
    event.preventDefault();
  });

  function mount() {
    if (ui || !document.body) { return false; }
    ui = build();
    document.body.appendChild(ui.host);
    sync();
    return true;
  }

  if (!mount()) {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  }
  // The main document is replaced wholesale on a cross-origin load, so wait for
  // it before deciding the bar cannot be attached.
  window.addEventListener("pageshow", function () {
    if (!ui && !mount()) {
      document.addEventListener("DOMContentLoaded", mount, { once: true });
    }
  });
})();
"#;
