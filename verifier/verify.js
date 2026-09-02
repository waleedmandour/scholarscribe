/* ScholarScribe Provenance Verifier — standalone, offline, no build step. */

(function () {
  "use strict";

  var GENESIS = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

  // ---------- tiny helpers ----------

  function $(id) { return document.getElementById(id); }

  function sha256HexBytes(bytes) {
    // js-sha256 exposes sha256.create / update for byte arrays.
    var h = sha256.create();
    h.update(bytes);
    return h.hex();
  }

  function hexToBytes(hex) {
    if (hex.length % 2 !== 0) throw new Error("odd-length hex");
    var out = new Uint8Array(hex.length / 2);
    for (var i = 0; i < out.length; i++) {
      out[i] = parseInt(hex.substr(i * 2, 2), 16);
    }
    return out;
  }

  function stripPrefix(s, prefix) {
    return s.indexOf(prefix) === 0 ? s.slice(prefix.length) : s;
  }

  function textEncoder() {
    return new TextEncoder(); // UTF-8, available in all modern browsers
  }

  // ---------- canonical forms (docs/PROVENANCE_SPEC.md §5–6) ----------

  function canonicalRecordString(r) {
    return [
      "scholarscribe-record-v1",
      r.session_id,
      r.author,
      String(r.start_time),
      String(r.end_time),
      r.snapshot_hash,
      String(r.chars_added),
      String(r.chars_removed),
      String(r.largest_insertion),
      r.prev_record_hash,
    ].join("\n");
  }

  function styleScoreLine(m) {
    var sc = m.style_consistency || {};
    if (sc.distance_score === null || sc.distance_score === undefined) return "null";
    // Must match Rust's format!("{:.4}", value) — 4 decimal places.
    return Number(sc.distance_score).toFixed(4);
  }

  function canonicalManifestPayload(m) {
    var lines = [];
    lines.push("SCHOLARSCRIBE-MANIFEST-v1");
    lines.push(m.version);
    lines.push(m.document_hash);
    lines.push(m.author_key_fingerprint);
    lines.push(String(m.generated_at));
    lines.push(m.generator);
    lines.push(String(m.revision_count));
    (m.sessions || []).forEach(function (s) {
      lines.push([
        s.session_id, s.author, String(s.start_time), String(s.end_time),
        s.snapshot_hash, String(s.chars_added), String(s.chars_removed),
        String(s.largest_insertion), s.prev_record_hash, s.record_hash,
      ].join("|"));
    });
    lines.push(styleScoreLine(m));
    lines.push((m.style_consistency || {}).baseline_source || "");
    lines.push(((m.style_consistency || {}).metrics_compared || []).join(","));
    lines.push(""); // yields the trailing "\n" exactly like the Rust builder
    return lines.join("\n");
  }

  // ---------- verification steps ----------

  function verifyChain(m) {
    var anomalies = [];
    var last = {}; // author -> previous record
    (m.sessions || []).forEach(function (r) {
      var recomputed = "sha256:" + sha256HexBytes(textEncoder().encode(canonicalRecordString(r)));
      if (recomputed !== r.record_hash) {
        anomalies.push("record hash mismatch for session " + r.session_id + " (author '" + r.author + "')");
      }
      if (!last[r.author]) {
        if (r.prev_record_hash !== GENESIS) {
          anomalies.push("first session " + r.session_id + " by '" + r.author + "' does not use the genesis marker");
        }
      } else {
        if (r.prev_record_hash !== last[r.author].record_hash) {
          anomalies.push("session " + r.session_id + " by '" + r.author + "' does not link to that author's previous record");
        }
        if (r.start_time < last[r.author].start_time) {
          anomalies.push("session " + r.session_id + " by '" + r.author + "' starts before the author's previous session");
        }
      }
      last[r.author] = r;
    });
    return { intact: anomalies.length === 0, anomalies: anomalies };
  }

  function verifySignature(m, pubkeyHex) {
    // 1. Fingerprint binding: fingerprint must equal sha256(public key).
    var fp = stripPrefix(m.author_key_fingerprint, "ed25519:");
    var fpCheck = sha256HexBytes(hexToBytes(pubkeyHex));
    if (fp !== fpCheck) {
      return { ok: false, error: "public key does not match the manifest fingerprint (expected sha256 " + fp + ", got " + fpCheck + ")" };
    }
    // 2. Signature check over the canonical payload.
    var sigHex = stripPrefix(m.signature, "ed25519:");
    if (!sigHex) return { ok: false, error: "manifest is not signed" };
    var msg = textEncoder().encode(canonicalManifestPayload(m));
    try {
      var sig = hexToBytes(sigHex);
      var pk = hexToBytes(pubkeyHex);
      var ok = nacl.sign.detached.verify(msg, sig, pk);
      return { ok: ok, error: ok ? null : "signature does not match this public key" };
    } catch (e) {
      return { ok: false, error: "signature check failed: " + e.message };
    }
  }

  function summarize(m) {
    var sessions = m.sessions || [];
    var min = Infinity, max = -Infinity, added = 0, removed = 0, largest = 0;
    var authors = {};
    sessions.forEach(function (s) {
      if (s.start_time < min) min = s.start_time;
      if (s.end_time > max) max = s.end_time;
      added += s.chars_added;
      removed += s.chars_removed;
      if (s.largest_insertion > largest) largest = s.largest_insertion;
      authors[s.author] = true;
    });
    return {
      count: sessions.length,
      authors: Object.keys(authors),
      spanHours: sessions.length ? Math.max(0, (max - min) / 3600) : 0,
      largestPct: added > 0 ? (largest / added) * 100 : 0,
      added: added,
      removed: removed,
    };
  }

  // ---------- rendering ----------

  var manifest = null;
  var zipEntries = null;
  var docHashResult = null;

  function verdictChip(ok, label) {
    return '<span class="chip ' + (ok ? "pass" : "fail") + '">' + label + (ok ? " ✓" : " ✗") + "</span>";
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }

  function render() {
    var panel = $("resultPanel");
    var out = $("result");
    if (!manifest) { panel.hidden = true; return; }
    panel.hidden = false;

    var chain = verifyChain(manifest);
    var pkHex = stripPrefix($("pubkey").value.trim(), "ed25519-pub:");
    var sig = null;
    if (pkHex) {
      try {
        sig = verifySignature(manifest, pkHex);
      } catch (e) {
        sig = { ok: false, error: e.message };
      }
    }
    var sum = summarize(manifest);

    var html = "";
    html += '<div class="chips">';
    html += verdictChip(chain.intact, "Hash chain intact");
    html += sig ? verdictChip(sig.ok, "Ed25519 signature") : '<span class="chip pending">Signature — awaiting public key</span>';
    html += docHashResult ? verdictChip(docHashResult.match, "Document binding") : "";
    html += "</div>";

    if (sig && !sig.ok && sig.error) html += '<p class="note err">' + escapeHtml(sig.error) + "</p>";
    if (docHashResult && !docHashResult.match) {
      html += '<p class="note err">document_hash mismatch: manifest says ' +
        escapeHtml(manifest.document_hash) + " but the provided file hashes to " +
        escapeHtml(docHashResult.actual) + "</p>";
    }

    html += "<table class=\"kv\">" +
      row("Version", manifest.version) +
      row("Generator", manifest.generator) +
      row("Generated at", new Date((manifest.generated_at || 0) * 1000).toISOString()) +
      row("Document hash", "<code>" + escapeHtml(manifest.document_hash) + "</code>") +
      row("Key fingerprint", "<code>" + escapeHtml(manifest.author_key_fingerprint) + "</code>") +
      row("Sessions", String(sum.count)) +
      row("Authors", escapeHtml(sum.authors.join(", "))) +
      row("Time span", sum.spanHours.toFixed(1) + " h") +
      row("Chars added / removed", sum.added.toLocaleString() + " / " + sum.removed.toLocaleString()) +
      row("Largest insertion", sum.largestPct.toFixed(1) + "% of added text") +
      row("Style distance", (manifest.style_consistency && manifest.style_consistency.distance_score !== null && manifest.style_consistency.distance_score !== undefined)
        ? Number(manifest.style_consistency.distance_score).toFixed(4) + " (" + escapeHtml(manifest.style_consistency.baseline_source) + ")"
        : "not computed (" + escapeHtml((manifest.style_consistency || {}).baseline_source || "unavailable") + ")") +
      "</table>";

    if (chain.anomalies.length) {
      html += '<p class="note err"><strong>Anomalies:</strong></p><ul>' +
        chain.anomalies.map(function (a) { return "<li>" + escapeHtml(a) + "</li>"; }).join("") + "</ul>";
    }

    html += "<h3>Sessions</h3><table class=\"sessions\"><thead><tr>" +
      "<th>#</th><th>Author</th><th>Start (UTC)</th><th>Duration</th><th>+chars</th><th>−chars</th><th>Record hash</th>" +
      "</tr></thead><tbody>";
    (manifest.sessions || []).forEach(function (s, i) {
      html += "<tr><td>" + (i + 1) + "</td><td>" + escapeHtml(s.author) + "</td><td>" +
        new Date(s.start_time * 1000).toISOString().replace("T", " ").slice(0, 16) + "</td><td>" +
        Math.max(0, Math.round((s.end_time - s.start_time) / 60)) + " min</td><td>" +
        s.chars_added.toLocaleString() + "</td><td>" + s.chars_removed.toLocaleString() +
        '</td><td class="mono">' + escapeHtml(s.record_hash.slice(0, 19)) + "…</td></tr>";
    });
    html += "</tbody></table>";

    html += '<p class="note">Interpretation bands for the style distance score are descriptive only — ' +
      "0.0–0.2 very close, 0.2–0.4 broadly consistent, 0.4–0.6 noticeable, 0.6–1.0 substantial. " +
      "This is <strong>not</strong> an AI-detection score.</p>";

    if (zipEntries) {
      var names = Object.keys(zipEntries);
      html += '<p class="muted small">Package contents: ' + names.map(escapeHtml).join(", ") + "</p>";
    }

    out.innerHTML = html;
  }

  function row(k, v) {
    return "<tr><th>" + k + "</th><td>" + v + "</td></tr>";
  }

  // ---------- file loading ----------

  function loadZip(file) {
    JSZip.loadAsync(file).then(function (zip) {
      zipEntries = {};
      var files = Object.keys(zip.files).filter(function (n) { return !zip.files[n].dir; });
      return Promise.all(files.map(function (name) {
        return zip.files[name].async("string").then(function (content) {
          zipEntries[name] = content;
        });
      })).then(function () {
        if (!zipEntries["manifest.json"]) throw new Error("manifest.json not found in the zip");
        manifest = JSON.parse(zipEntries["manifest.json"]);
        docHashResult = null;
        render();
      });
    }).catch(function (e) {
      alert("Could not read the package: " + e.message);
    });
  }

  function loadManifestJson(file) {
    var fr = new FileReader();
    fr.onload = function () {
      try {
        manifest = JSON.parse(fr.result);
        zipEntries = null;
        docHashResult = null;
        render();
      } catch (e) {
        alert("manifest.json is not valid JSON: " + e.message);
      }
    };
    fr.readAsText(file);
  }

  function loadPubKeyFile(file) {
    var fr = new FileReader();
    fr.onload = function () {
      try {
        var obj = JSON.parse(fr.result);
        var pk = obj.public_key || obj.fingerprint || String(fr.result);
        $("pubkey").value = pk;
        render();
      } catch (e) {
        // maybe a bare hex string
        $("pubkey").value = String(fr.result).trim();
        render();
      }
    };
    fr.readAsText(file);
  }

  function bindDocument(file) {
    if (!manifest) { alert("Load the package first."); return; }
    var name = file.name.toLowerCase();
    if (name.endsWith(".docx")) {
      JSZip.loadAsync(file).then(function (zip) {
        var entry = zip.file("word/document.xml");
        if (!entry) throw new Error("word/document.xml not found in the .docx");
        return entry.async("arraybuffer");
      }).then(function (buf) {
        var actual = "sha256:" + sha256HexBytes(new Uint8Array(buf));
        docHashResult = { actual: actual, match: actual === manifest.document_hash };
        render();
      }).catch(function (e) {
        alert("Could not read the .docx: " + e.message);
      });
    } else {
      var fr = new FileReader();
      fr.onload = function () {
        var bytes = new Uint8Array(fr.result);
        var actual = "sha256:" + sha256HexBytes(bytes);
        docHashResult = { actual: actual, match: actual === manifest.document_hash };
        render();
      };
      fr.readAsArrayBuffer(file);
    }
  }

  // ---------- wiring ----------

  $("zipInput").addEventListener("change", function (e) {
    if (e.target.files.length) loadZip(e.target.files[0]);
  });
  $("manifestInput").addEventListener("change", function (e) {
    if (e.target.files.length) loadManifestJson(e.target.files[0]);
  });
  $("pubkeyFile").addEventListener("change", function (e) {
    if (e.target.files.length) loadPubKeyFile(e.target.files[0]);
  });
  $("docInput").addEventListener("change", function (e) {
    if (e.target.files.length) bindDocument(e.target.files[0]);
  });
  $("pubkey").addEventListener("input", render);
})();
