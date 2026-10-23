// ScholarScribe Word Assist — legacy-webview fallback (A2 integration).
//
// This is a CLASSIC script (ES5, no modules) served by the engine and
// loaded BEFORE the ES-module entry. Webviews without ES-module support
// silently ignore <script type="module">, which used to leave the pane
// stuck on "Starting…" with no explanation; this file turns that into a
// clear message instead of a blank pane.
//
// Detection: `noModule` on HTMLScriptElement is the standard ES-module
// support probe. Windows M365 (Edge WebView2/Chromium) and Word-for-Mac
// (WKWebView/Safari) both support modules; legacy perpetual Word builds
// with the IE11/legacy-Edge webview do not (per Microsoft's "Browsers
// used by Office Add-ins" doc — verified 2026-10-23). Which exact Word
// builds fall back to which engine on the owner's machines is spike
// (g)/(l): NOT VERIFIED until OBSERVATIONS.md lands.
(function () {
  "use strict";
  var supported = false;
  try {
    supported = "noModule" in document.createElement("script");
  } catch (e) {
    supported = false;
  }
  if (!supported) {
    var el = document.getElementById("status");
    if (el) {
      el.className = "status err";
      el.textContent =
        "This Word window is using an older webview that cannot run the " +
        "Word Assist pane (no ES-module support). Update Microsoft 365 or " +
        "Word so add-ins use the modern webview (Edge WebView2 on Windows, " +
        "WKWebView on macOS), then reopen this pane.";
    }
    var log = document.getElementById("log");
    if (log && log.parentElement) {
      var note = document.createElement("li");
      note.textContent = "legacy webview: ES modules unavailable; pane disabled";
      log.appendChild(note);
    }
  }
})();
