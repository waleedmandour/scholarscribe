# Word Assist spike — owner observation checklist

Purpose: close the Phase 0 unknowns that only a live Word on Windows can close.
Everything you see is logged with timestamps in the pane; use **Copy log** and
paste results under "Observed" below. A "fail" result here is a *design input*,
not a problem — several items exist precisely to record a negative.

**Environment needed:** Word on Windows, Microsoft 365 subscription (the
annotation APIs are documented to require it — RECON §4 V2), this repo checked
out, Node.js available.

## Step 0 — run the spike

1. In a terminal: `node word-addin/spike/serve.mjs` (binds 127.0.0.1:38080).
   Leave it running; its stdout logs every request the add-in makes (Origin /
   Host / User-Agent lines — keep a copy, they are observation (g)/(h) input).
2. Word → **Insert → My Add-ins** (dropdown) → **UPLOAD MY ADD-IN…** → pick
   `word-addin/spike/manifest.xml`.
3. Home ribbon → group "Word Assist spike" → **Spike pane**.
   - If Word refuses the pane with a certificate/security error at this point,
     that IS observation (g): record the exact message and stop here.
4. The pane shows an Environment block — copy it into (f) and (l) below.

## Observations

Fill "Observed" for each row (paste log excerpts where useful).

| ID | What to check | How | Expected candidates | Observed |
|----|----------------|-----|---------------------|----------|
| f | Requirement sets | Read the Environment block | WordApi 1.5/1.6/1.7/1.8 booleans + platform + userAgent | |
| l | Webview runtime & loopback reachability | userAgent line + whether the pane loaded at all over http | Edge WebView2 version string; pane loads with no OS exemption | |
| g | Is plain `http://localhost:38080` accepted? | Whether the pane loads (step 0.3) | loads / mixed-content or trust error (record exact text) | |
| h | Traffic while the pane loads + annotations render | serve.mjs stdout (our requests) + optional Wireshark/Financial capture of loopback & CDN — optional, your choice | pane requests taskpane.html/js/icon from us; office.js from appsforoffice.microsoft.com; observe whether the annotation popup triggers traffic to any Microsoft host (the 1.7 docs say an "underlying service" is involved — seeing it beats assuming it) | |

Step 1 — click **Plant test paragraph**, then **Read it back (getText)**:

| ID | What to check | How | Expected candidates | Observed |
|----|----------------|-----|---------------------|----------|
| a | Offset units | The planted text mixes ASCII, `é`/`“”` (BMP), NBSP, Arabic, and astral emoji (👍🏽 🎉). Pane shows `string.length` (UTF-16 units) vs code points and a per-character index map. Then insert an annotation with **start/length inputs** around a chosen character and see WHERE Word underlines. E.g. annotate start=0 length=1, then start around the emoji, then inside the Arabic word | underline lands on the character you aimed at under **UTF-16 code-unit** indexing, or shifts by 1 per astral emoji if Word counts **code points** | |
| m | getText option support | The pane logs whether `getText({includeHiddenText, includeDeletedText})` is accepted | accepted (returns identical text) / REJECTED with message | |

Step 2 — click **Register all event handlers**, then **Insert hardcoded annotation (0..12)**:

| ID | What to check | How | Expected candidates | Observed |
|----|----------------|-----|---------------------|----------|
| j | Annotation↔id correlation | After insert, the pane logs `annotations.item id=… state=…` and the `onAnnotationInserted` args | our critique maps to a Word-assigned id; record the mapping mechanism | |
| e | Do OUR own actions fire events? | Watch `onParagraphChanged` / `onAnnotationInserted` / `onAnnotationRemoved` right after the insert (no user edit) | insertAnnotations fires AnnotationInserted (yes/no); whether it also fires ParagraphChanged | |
| c | Multiple suggestions | Insert 2–3 annotations at different offsets via the custom inputs | all render as underlines; popups independent? overlapping allowed? | |
| d | Annotation drift while typing | Type/delete characters inside and before the annotated ranges; watch ParagraphChanged args + whether underlines stay anchored | Word re-anchors annotations with the text (good) or leaves stale offsets (bad) | |
| m | Contexts beyond body text | Repeat the hardcoded annotation inside: a table cell, a footnote, a header, a text box, and inside a content control; also with **Track Changes ON** (with and without a tracked deletion inside the annotated range) | annotations work in body+tables; note which contexts reject or misplace them; note whether tracked deletions change offsets vs the getText options result | |

Step 3 — with an annotation visible, click its underline to open the popup, then click the suggestion "Spike: replace with «FIXED»" **with Apply-on-action OFF**:

| ID | What to check | How | Expected candidates | Observed |
|----|----------------|-----|---------------------|----------|
| b | Who applies the replacement? | Watch the document text and the `onAnnotationPopupAction` log | Word applies it automatically (text changes, annotation clears) OR Word only fires the event and closes the popup, text unchanged → the add-in must apply | |
| b2 | Popup action payload | The log line shows `action` and `critiqueSuggestion` values | record the exact enum string (docs do not state it — RECON §4 V3) | |

Repeat step 3 with **Apply-on-action ON**: the pane deletes the annotation and
logs whether the text had already changed — confirming the (b) result from the
other side.

## When done

1. Paste the full pane log and the serve.mjs request lines into this file (or
   reply with them).
2. Delete the spike add-in: Insert → My Add-ins → see all → remove; stop the
   server with Ctrl+C. Nothing is installed system-wide; no trust store or
   registry change was made by the spike.
