#!/usr/bin/env python3
"""Generate a 2-page Quick Start Guide for ScholarScribe.

Designed for distribution at academic writing workshops. Two A4 pages:
  Page 1: Logo + title + what-is-ScholarScribe + 7 feature groups + privacy guarantee
  Page 2: Quick start (5 steps) + how to cite + ethical use + footer

Professional light theme matching the User Guide PDF. App logo embedded.
Author: Dr. Waleed Mandour. Citation in APA + BibTeX.
"""

from __future__ import annotations

import sys
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.units import mm
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    PageTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
    PageBreak,
    HRFlowable,
    Image,
    NextPageTemplate,
)
from reportlab.platypus.flowables import Flowable

# ---------- Palette ----------
C_BG = colors.HexColor("#FFFFFF")
C_TEXT = colors.HexColor("#1a1d23")
C_MUTED = colors.HexColor("#5c6470")
C_DIM = colors.HexColor("#8a929e")
C_ACCENT = colors.HexColor("#2a5bd7")
C_ACCENT_DARK = colors.HexColor("#1f4abf")
C_ACCENT_SOFT = colors.HexColor("#e7eefb")
C_BORDER = colors.HexColor("#dcdfe3")
C_TABLE_HDR = colors.HexColor("#f0f1f3")
C_TABLE_ALT = colors.HexColor("#fafbfc")
C_SUCCESS = colors.HexColor("#1a8a52")
C_WARNING = colors.HexColor("#b76e00")

FONT_SANS = "Helvetica"
FONT_SERIF = "Times-Roman"
FONT_MONO = "Courier"

# ---------- Styles ----------

def build_styles():
    s = {}
    s["title"] = ParagraphStyle("title", fontName=FONT_SANS, fontSize=22, leading=28,
        textColor=C_ACCENT_DARK, alignment=1, spaceAfter=2)
    s["subtitle"] = ParagraphStyle("subtitle", fontName=FONT_SERIF, fontSize=11, leading=15,
        textColor=C_MUTED, alignment=1, spaceAfter=10)
    s["author"] = ParagraphStyle("author", fontName=FONT_SANS, fontSize=9, leading=12,
        textColor=C_DIM, alignment=1, spaceAfter=2)
    s["h2"] = ParagraphStyle("h2", fontName=FONT_SANS, fontSize=13, leading=17,
        textColor=C_ACCENT, spaceBefore=10, spaceAfter=4)
    s["body"] = ParagraphStyle("body", fontName=FONT_SERIF, fontSize=9.5, leading=13,
        textColor=C_TEXT, spaceAfter=4)
    s["body_sm"] = ParagraphStyle("body_sm", fontName=FONT_SERIF, fontSize=8.5, leading=12,
        textColor=C_TEXT, spaceAfter=2)
    s["feature_h"] = ParagraphStyle("feature_h", fontName=FONT_SANS, fontSize=8.5, leading=11,
        textColor=C_ACCENT_DARK, spaceAfter=1)
    s["feature_d"] = ParagraphStyle("feature_d", fontName=FONT_SERIF, fontSize=8, leading=11,
        textColor=C_MUTED, spaceAfter=2)
    s["step"] = ParagraphStyle("step", fontName=FONT_SERIF, fontSize=9, leading=12,
        textColor=C_TEXT, spaceAfter=3)
    s["cite"] = ParagraphStyle("cite", fontName=FONT_SERIF, fontSize=8.5, leading=12,
        textColor=C_TEXT, spaceAfter=4)
    s["cite_bib"] = ParagraphStyle("cite_bib", fontName=FONT_MONO, fontSize=7.5, leading=10,
        textColor=C_TEXT, backColor=colors.HexColor("#f6f7f9"), borderPadding=(4,6,4,6),
        spaceAfter=4)
    s["callout"] = ParagraphStyle("callout", fontName=FONT_SERIF, fontSize=8.5, leading=12,
        textColor=C_TEXT, backColor=C_ACCENT_SOFT, borderPadding=(6,8,6,8), spaceAfter=4)
    s["footer"] = ParagraphStyle("footer", fontName=FONT_SANS, fontSize=7.5, leading=10,
        textColor=C_DIM, alignment=1)
    return s

# ---------- Page decorations ----------

def page1_decor(canvas, doc):
    canvas.saveState()
    # Top accent bar
    canvas.setFillColor(C_ACCENT)
    canvas.rect(0, A4[1] - 3*mm, A4[0], 3*mm, fill=1, stroke=0)
    # Bottom rule
    canvas.setStrokeColor(C_BORDER)
    canvas.setLineWidth(0.3)
    canvas.line(18*mm, 12*mm, A4[0]-18*mm, 12*mm)
    # Footer text
    canvas.setFillColor(C_DIM)
    canvas.setFont(FONT_SANS, 7)
    canvas.drawString(18*mm, 8*mm, "ScholarScribe v2.2.1 - Quick Start Guide")
    canvas.drawRightString(A4[0]-18*mm, 8*mm, "Page 1 of 2")
    canvas.restoreState()

def page2_decor(canvas, doc):
    canvas.saveState()
    canvas.setFillColor(C_ACCENT)
    canvas.rect(0, A4[1] - 3*mm, A4[0], 3*mm, fill=1, stroke=0)
    canvas.setStrokeColor(C_BORDER)
    canvas.setLineWidth(0.3)
    canvas.line(18*mm, 12*mm, A4[0]-18*mm, 12*mm)
    canvas.setFillColor(C_DIM)
    canvas.setFont(FONT_SANS, 7)
    canvas.drawString(18*mm, 8*mm, "ScholarScribe v2.2.1 - Quick Start Guide")
    canvas.drawRightString(A4[0]-18*mm, 8*mm, "Page 2 of 2")
    canvas.restoreState()

# ---------- Content ----------

def build_page1(styles, logo_path):
    flow = []
    # Logo centered at top
    if logo_path and logo_path.exists():
        try:
            logo = Image(str(logo_path), width=22*mm, height=22*mm)
            logo.hAlign = "CENTER"
            flow.append(logo)
            flow.append(Spacer(1, 4*mm))
        except Exception:
            pass

    # Thin accent rule
    flow.append(HRFlowable(width="30%", thickness=1.0, color=C_ACCENT,
        spaceBefore=0, spaceAfter=6, hAlign="CENTER"))

    flow.append(Paragraph("ScholarScribe", styles["title"]))
    flow.append(Paragraph("Quick Start Guide", styles["subtitle"]))
    flow.append(Paragraph("Designed and directed by Dr. Waleed Mandour", styles["author"]))
    flow.append(Paragraph("v2.2.1 - Windows - macOS - Linux - MIT License", styles["author"]))
    flow.append(Spacer(1, 4*mm))

    # What is ScholarScribe
    flow.append(Paragraph("What is ScholarScribe?", styles["h2"]))
    flow.append(Paragraph(
        "ScholarScribe is a privacy-first, local-LLM writing companion for researchers. "
        "It runs entirely on your device with zero telemetry, no cloud calls, and no paid APIs. "
        "It helps you draft, clean, validate, and disclose your manuscript using open LLMs "
        "(Gemma 3, Qwen 3, Phi-4, DeepSeek R1, Llama 3.3) that run locally via Ollama. "
        "The sidebar organizes 20 tools into 7 workflow phases, from getting started to "
        "privacy and app management.",
        styles["body"]))
    flow.append(Spacer(1, 3*mm))

    # 20 tools in a 3-column table: Group | Tool | Brief explanation
    flow.append(Paragraph("The 20 tools at a glance (7 workflow phases)", styles["h2"]))

    tools = [
        ("Get started", "Models", "Install, import, and manage local LLMs (8-64 GB RAM)."),
        ("Prepare the draft", "Text Cleaner", "Fix 24 PDF/OCR/web artifacts in .txt, .md, or .docx."),
        ("Prepare the draft", "Citations", "Validate in-text citations against .bib or pasted references."),
        ("Understand the draft", "Stats", "Word count, readability, journal-target comparison."),
        ("Understand the draft", "Structure", "Heading tree, missing-section suggestions, per-section excerpts."),
        ("AI writing help", "Abstract", "LLM-generated Background/Methods/Results/Conclusions abstract."),
        ("AI writing help", "Writing Coach", "Socratic coaching on a paragraph or argument (never rewrites)."),
        ("AI writing help", "Chat", "Local-only chat with file picker to attach manuscript for context."),
        ("Authenticity and style", "Detector Literacy", "How AI detectors work and where they fail, with cited stats."),
        ("Authenticity and style", "Risk Profile", "4 proxy signals per passage, click-to-expand detail panel."),
        ("Authenticity and style", "Style Analysis", "Compare a draft to your own prior writing (12 metrics)."),
        ("Authenticity and style", "Fingerprint", "Multi-paper stylistic fingerprint of your writing baseline."),
        ("Authenticity and style", "Voice Check", "Flag within-document stylistic shifts with excerpts."),
        ("Evidence and compliance", "Journal", "Auto-saved timestamped snapshots of your draft."),
        ("Evidence and compliance", "Provenance", "Export signed, hash-chained evidence of your revision history."),
        ("Evidence and compliance", "Appeal Letter", "Generate an evidence-based appeal if falsely flagged."),
        ("Evidence and compliance", "Disclosure", "Generate venue-compliant AI-use disclosure statements."),
        ("Privacy and app", "Privacy Audit", "Live log of every file read and outbound HTTP call."),
        ("Privacy and app", "Saved Work", "Opt-in local JSON persistence (off by default)."),
        ("Privacy and app", "About", "Version, environment, developer credits."),
    ]

    rows = []
    for group, tool, desc in tools:
        rows.append([
            Paragraph(f"<b>{group}</b>", styles["feature_h"]),
            Paragraph(f"<b>{tool}</b>", styles["feature_h"]),
            Paragraph(desc, styles["feature_d"]),
        ])

    content_w = A4[0] - 2 * 18 * mm
    col_widths = [content_w * 0.22, content_w * 0.20, content_w * 0.58]
    t = Table(rows, colWidths=col_widths, hAlign="LEFT")
    t.setStyle(TableStyle([
        ("VALIGN", (0,0), (-1,-1), "TOP"),
        ("LEFTPADDING", (0,0), (-1,-1), 0),
        ("RIGHTPADDING", (0,0), (-1,-1), 4),
        ("TOPPADDING", (0,0), (-1,-1), 3),
        ("BOTTOMPADDING", (0,0), (-1,-1), 3),
        ("LINEBELOW", (0,0), (-1,-2), 0.3, C_BORDER),
    ]))
    flow.append(t)
    flow.append(Spacer(1, 3*mm))

    # Privacy guarantee callout
    flow.append(Paragraph(
        "<b>Privacy guarantee.</b> Nothing you write, paste, or open ever leaves your device. "
        "Drafts, .bib files, chat messages, and reference samples stay in memory or in local "
        "JSON files. The only outbound network call is to registry.ollama.ai when you choose "
        "to download a model, and that call carries no text, no prompts, no usage data. "
        "Verify this yourself: the Privacy Audit tab logs every file read and outbound HTTP "
        "call in real time.",
        styles["callout"]))

    flow.append(PageBreak())
    return flow

def build_page2(styles):
    flow = []

    # Quick start
    flow.append(Paragraph("Quick start (5 steps, under 10 minutes)", styles["h2"]))

    steps = [
        ("Install Ollama",
         "Download from ollama.com/download (~150 MB). Run the installer. Look for the llama "
         "icon in your system tray."),
        ("Install ScholarScribe",
         "Download the installer for your platform from waleedmandour.org/projects/scholarscribe: .msi or .exe "
         "(Windows), .dmg (macOS), .deb or .AppImage (Linux). The sidebar should show a green "
         "\"Ollama backend: running\" pill."),
        ("Download a model",
         "Open the Models tab. Pick a model that fits your RAM (8 GB: Gemma 3 4B; 16 GB: Gemma "
         "3 12B; 32 GB: Gemma 3 27B). Click Download. Wait 2-15 minutes."),
        ("Paste your draft",
         "Open any tool tab (Text Cleaner, Stats, Structure, Risk Profile, Voice Check, "
         "Citations). Paste your draft or click \"Open file\" to load a .txt, .md, .tex, or "
         ".docx file."),
        ("Use the tools",
         "Clean text, validate citations (paste references or load .bib), check structure and "
         "voice consistency, assess authenticity risk, generate an abstract, get Socratic "
         "coaching from the Chat tab (attach your manuscript for context)."),
    ]

    for i, (title, desc) in enumerate(steps, 1):
        flow.append(Paragraph(
            f"<b>{i}. {title}.</b> {desc}",
            styles["step"]))
        flow.append(Spacer(1, 1*mm))

    flow.append(Spacer(1, 3*mm))

    # How to cite
    flow.append(Paragraph("How to cite this tool", styles["h2"]))
    flow.append(Paragraph(
        "<b>APA (7th edition):</b>",
        styles["cite"]))
    flow.append(Paragraph(
        "Mandour, W. (2026). <i>ScholarScribe</i> (Version 2.2.1) "
        "[Computer software]. Zenodo. "
        '<link href="https://doi.org/10.5281/zenodo.20781043">'
        '<font color="#2a5bd7">https://doi.org/10.5281/zenodo.20781043</font></link>',
        styles["cite"]))
    flow.append(Spacer(1, 2*mm))
    flow.append(Paragraph(
        "<b>BibTeX:</b>",
        styles["cite"]))
    flow.append(Paragraph(
        '@software{mandour2026scholarscribe,<br/>'
        '&nbsp;&nbsp;author = {Mandour, Waleed},<br/>'
        '&nbsp;&nbsp;title = {ScholarScribe},<br/>'
        '&nbsp;&nbsp;version = {2.2.1},<br/>'
        '&nbsp;&nbsp;year = {2026},<br/>'
        '&nbsp;&nbsp;url = {https://waleedmandour.org/projects/scholarscribe},<br/>'
        '&nbsp;&nbsp;doi = {10.5281/zenodo.20781043}<br/>'
        '}',
        styles["cite_bib"]))

    flow.append(Spacer(1, 3*mm))

    # Ethical use
    flow.append(Paragraph("Ethical use", styles["h2"]))
    flow.append(Paragraph(
        "ScholarScribe is designed for researchers who have genuinely written their manuscript "
        "and want transparent, local AI assistance. The Chat tab is a <b>Socratic coach, not a "
        "ghostwriter</b>: it asks guiding questions and never rewrites your text. The Risk "
        "Profile and Voice Check tabs surface stylistic patterns for awareness, not for evading "
        "AI detectors. If you used AI assistance, disclose it via the Disclosure tab. See "
        "docs/ETHICS.md for the full policy.",
        styles["body"]))

    flow.append(Spacer(1, 3*mm))

    # Contact and identifiers
    flow.append(Paragraph("Contact and identifiers", styles["h2"]))
    contact_rows = [
        [Paragraph("<b>Author</b>", styles["feature_h"]),
         Paragraph("Dr. Waleed Mandour", styles["feature_d"])],
        [Paragraph("<b>Email</b>", styles["feature_h"]),
         Paragraph('<link href="mailto:w.abumandour@squ.edu.om"><font color="#2a5bd7">w.abumandour@squ.edu.om</font></link>', styles["feature_d"])],
        [Paragraph("<b>ORCID</b>", styles["feature_h"]),
         Paragraph('<link href="https://orcid.org/0000-0002-9262-5993"><font color="#2a5bd7">0000-0002-9262-5993</font></link>', styles["feature_d"])],
        [Paragraph("<b>DOI</b>", styles["feature_h"]),
         Paragraph('<link href="https://doi.org/10.5281/zenodo.20781043"><font color="#2a5bd7">10.5281/zenodo.20781043</font></link>', styles["feature_d"])],
        [Paragraph("<b>Website</b>", styles["feature_h"]),
         Paragraph('<link href="https://waleedmandour.org/projects/scholarscribe"><font color="#2a5bd7">waleedmandour.org/projects/scholarscribe</font></link>', styles["feature_d"])],
    ]
    content_w = A4[0] - 2 * 18 * mm
    ct = Table(contact_rows, colWidths=[content_w * 0.20, content_w * 0.80], hAlign="LEFT")
    ct.setStyle(TableStyle([
        ("VALIGN", (0,0), (-1,-1), "TOP"),
        ("LEFTPADDING", (0,0), (-1,-1), 0),
        ("RIGHTPADDING", (0,0), (-1,-1), 4),
        ("TOPPADDING", (0,0), (-1,-1), 2),
        ("BOTTOMPADDING", (0,0), (-1,-1), 2),
        ("LINEBELOW", (0,0), (-1,-2), 0.3, C_BORDER),
    ]))
    flow.append(ct)

    flow.append(Spacer(1, 4*mm))

    # Footer
    flow.append(HRFlowable(width="100%", thickness=0.5, color=C_BORDER, spaceAfter=4))
    flow.append(Paragraph(
        "ScholarScribe v2.2.1 - (c) 2026 Dr. Waleed Mandour - MIT License - "
        "waleedmandour.org/projects/scholarscribe - DOI: 10.5281/zenodo.20781043",
        styles["footer"]))

    return flow

# ---------- Main ----------

def main():
    repo_root = Path(__file__).resolve().parent.parent
    logo_candidates = [
        repo_root / "src-tauri" / "icons" / "128x128@2x.png",
        repo_root / "src-tauri" / "icons" / "128x128.png",
        repo_root / "src-tauri" / "icons" / "icon.png",
    ]
    logo_path = next((p for p in logo_candidates if p.exists()), None)

    output = Path("/home/z/my-project/download/ScholarScribe-Quick-Guide.pdf")
    output.parent.mkdir(parents=True, exist_ok=True)

    styles = build_styles()

    doc = BaseDocTemplate(
        str(output),
        pagesize=A4,
        leftMargin=18*mm, rightMargin=18*mm,
        topMargin=14*mm, bottomMargin=14*mm,
        title="ScholarScribe Quick Start Guide",
        author="Dr. Waleed Mandour",
        subject="ScholarScribe v2.2.1 Quick Guide",
        creator="ScholarScribe build pipeline",
    )

    frame = Frame(18*mm, 14*mm, A4[0]-36*mm, A4[1]-28*mm,
        leftPadding=0, rightPadding=0, topPadding=0, bottomPadding=0, id="main")

    doc.addPageTemplates([
        PageTemplate(id="p1", frames=[frame], onPage=page1_decor),
        PageTemplate(id="p2", frames=[frame], onPage=page2_decor),
    ])

    story = []
    story.append(NextPageTemplate("p1"))
    story.extend(build_page1(styles, logo_path))
    # NextPageTemplate must come BEFORE the PageBreak (which is the
    # last element of build_page1) so it takes effect on page 2.
    # Since build_page1 already includes PageBreak, we insert the
    # template switch just before it by rebuilding the story.
    # (ReportLab applies NextPageTemplate on the NEXT page boundary.)
    story.insert(-1, NextPageTemplate("p2"))
    story.extend(build_page2(styles))

    doc.build(story)

    size_kb = output.stat().st_size / 1024
    print(f"Wrote {output} ({size_kb:.1f} KB)")

    try:
        from pypdf import PdfReader
        r = PdfReader(str(output))
        print(f"Pages: {len(r.pages)}")
    except Exception as e:
        print(f"Could not count pages: {e}")

if __name__ == "__main__":
    sys.exit(main())
