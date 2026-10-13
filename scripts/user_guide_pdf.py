#!/usr/bin/env python3
"""Generate the 2-page ScholarScribe-User-Guide.pdf from USER_GUIDE.md.

A compact ReportLab generator that parses the markdown structure of
USER_GUIDE.md and renders it as a polished two-page A4 PDF:

  - Compact title band on page 1 (logo, title, pitch, version line),
    NOT a separate cover page: the whole document is exactly 2 pages.
  - Body sections with H1/H2 headings
  - Tables (markdown pipe tables) with content-aware column widths;
    "group rows" (bold second cell, empty first cell) render as
    full-width section bands inside the table
  - Paragraphs (bold/italic/code/links support), numbered and bulleted
    lists, blockquotes rendered as centered muted lines (header only)

All fonts are ReportLab built-ins (Helvetica / Times / Courier), so the
layout is deterministic across reportlab versions and platforms: the
CI-generated PDF is page-for-page identical to a local build.

Outputs to ScholarScribe-User-Guide.pdf (or --output), matching the
filename pattern of previous releases.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.pdfbase import pdfmetrics
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    PageTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
    KeepTogether,
    HRFlowable,
    ListFlowable,
    ListItem,
    Image,
)
from reportlab.platypus.flowables import Flowable

# ---------- Fonts (built-ins only: deterministic layout everywhere) ----------

FONT_FAMILY_SANS = "Helvetica"
FONT_FAMILY_SERIF = "Times-Roman"
FONT_FAMILY_MONO = "Courier"

pdfmetrics.registerFontFamily(
    "Helvetica", normal="Helvetica", bold="Helvetica-Bold",
    italic="Helvetica-Oblique", boldItalic="Helvetica-BoldOblique",
)

# ---------- Palette ----------
# Calm, scholarly, accessible palette. WCAG-AA contrast verified.
# Light theme throughout: a clean, professional white background with a
# single accent color for headings and rules.
PALETTE = {
    "bg": colors.HexColor("#FFFFFF"),
    "text": colors.HexColor("#1a1d23"),         # body text, near-black
    "text_muted": colors.HexColor("#5c6470"),   # secondary text
    "text_dim": colors.HexColor("#8a929e"),     # tertiary text
    "accent": colors.HexColor("#2a5bd7"),       # ScholarScribe blue
    "accent_dark": colors.HexColor("#1f4abf"),  # darker blue for the title
    "accent_soft": colors.HexColor("#e7eefb"),  # table group-band background
    "border": colors.HexColor("#dcdfe3"),       # table borders
    "table_header_bg": colors.HexColor("#f0f1f3"),
    "table_alt_row": colors.HexColor("#fafbfc"),
    "code_bg": colors.HexColor("#f0f1f3"),
}

# ---------- Page geometry ----------

MARGIN_LR = 14 * mm
MARGIN_TOP = 11.5 * mm
MARGIN_BOTTOM = 13.5 * mm
CONTENT_WIDTH = A4[0] - 2 * MARGIN_LR


# ---------- Styles ----------

def build_styles() -> dict[str, ParagraphStyle]:
    base = getSampleStyleSheet()
    styles = {}

    styles["body"] = ParagraphStyle(
        "body",
        parent=base["BodyText"],
        fontName=FONT_FAMILY_SERIF,
        fontSize=9.2,
        leading=12.0,
        textColor=PALETTE["text"],
        spaceBefore=2.5,
        spaceAfter=2.5,
    )

    # The single H1 is the document title at the top of page 1.
    styles["h1"] = ParagraphStyle(
        "h1",
        parent=base["Heading1"],
        fontName=FONT_FAMILY_SANS,
        fontSize=20,
        leading=24,
        textColor=PALETTE["accent_dark"],
        alignment=1,  # center
        spaceBefore=2,
        spaceAfter=2,
    )

    styles["h2"] = ParagraphStyle(
        "h2",
        parent=base["Heading2"],
        fontName=FONT_FAMILY_SANS,
        fontSize=11.5,
        leading=14,
        textColor=PALETTE["accent"],
        spaceBefore=7,
        spaceAfter=2.5,
    )

    styles["h3"] = ParagraphStyle(
        "h3",
        parent=base["Heading3"],
        fontName=FONT_FAMILY_SANS,
        fontSize=10,
        leading=13,
        textColor=PALETTE["text"],
        spaceBefore=7,
        spaceAfter=3,
    )

    # Blockquotes render as centered muted lines; the guide uses them only
    # in the header band (pitch + version line).
    styles["blockquote"] = ParagraphStyle(
        "blockquote",
        parent=styles["body"],
        alignment=1,
        textColor=PALETTE["text_muted"],
        spaceBefore=1,
        spaceAfter=1,
    )

    styles["li"] = ParagraphStyle(
        "li",
        parent=styles["body"],
        leftIndent=12,
        spaceBefore=1,
        spaceAfter=1,
    )

    styles["table_cell"] = ParagraphStyle(
        "table_cell",
        parent=styles["body"],
        fontSize=8.1,
        leading=10.4,
        spaceBefore=0,
        spaceAfter=0,
    )

    styles["table_header"] = ParagraphStyle(
        "table_header",
        parent=styles["table_cell"],
        fontName=FONT_FAMILY_SANS,
        textColor=PALETTE["text_muted"],
        textTransform="uppercase",
        fontSize=7.8,
    )

    styles["table_group"] = ParagraphStyle(
        "table_group",
        parent=styles["table_cell"],
        fontName=FONT_FAMILY_SANS,
        fontSize=8.1,
        leading=10.4,
        textColor=PALETTE["accent_dark"],
    )

    return styles


# ---------- Inline markdown -> ReportLab markup ----------

def render_inline(text: str) -> str:
    """Convert a line of inline markdown into ReportLab Paragraph markup.

    Handles: **bold**, *italic*, `code`, [link](url), <url>, escapes &, <, >.
    """
    # Escape XML special chars first (except where we will inject tags)
    text = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

    # Inline code: `code` -> <font face="Courier" backColor="...">code</font>
    text = re.sub(
        r"`([^`]+)`",
        r'<font face="Courier" size="8" backColor="#f0f1f3">\1</font>',
        text,
    )
    # Bold: **text** -> <b>text</b>
    text = re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", text)
    # Italic: *text* -> <i>text</i>
    text = re.sub(r"\*([^*]+)\*", r"<i>\1</i>", text)
    # Links: [text](url) -> <link href="url"><font color="...">text</font></link>
    text = re.sub(
        r"\[([^\]]+)\]\(([^)]+)\)",
        r'<link href="\2"><font color="#2a5bd7">\1</font></link>',
        text,
    )
    # Bare URL wrapped in angle brackets: <https://...> -> clickable link
    text = re.sub(
        r"&lt;(https?://[^&\s]+)&gt;",
        r'<link href="\1"><font color="#2a5bd7">\1</font></link>',
        text,
    )
    # Plain URL (no angle brackets): https://... -> clickable
    text = re.sub(
        r"(?<![\"'\=\&])(https?://[A-Za-z0-9.\-/_]+)",
        r'<link href="\1"><font color="#2a5bd7">\1</font></link>',
        text,
    )
    return text


# ---------- Markdown parser -> flowables ----------

def parse_markdown_to_flowables(md_text: str, styles: dict) -> list:
    """Parse USER_GUIDE.md content (without the cover header lines) into
    a list of ReportLab flowables."""
    flowables: list = []
    lines = md_text.split("\n")
    i = 0
    n = len(lines)

    while i < n:
        line = lines[i]

        # Skip blank lines
        if not line.strip():
            i += 1
            continue

        # Skip horizontal rules
        if line.strip() in ("---", "***", "___"):
            flowables.append(HRFlowable(
                width="100%",
                thickness=0.5,
                color=PALETTE["border"],
                spaceBefore=4,
                spaceAfter=4,
            ))
            i += 1
            continue

        # Headings
        m = re.match(r"^(#{1,6})\s+(.+)$", line)
        if m:
            level = len(m.group(1))
            text = render_inline(m.group(2).strip())
            style_key = f"h{min(level, 3)}"
            flowables.append(Paragraph(text, styles[style_key]))
            i += 1
            continue

        # Blockquote: lines starting with '> '
        if line.lstrip().startswith(">"):
            quote_lines = []
            while i < n and lines[i].lstrip().startswith(">"):
                quote_lines.append(re.sub(r"^\s*>\s?", "", lines[i]))
                i += 1
            for q in quote_lines:
                if q.strip():
                    flowables.append(Paragraph(render_inline(q), styles["blockquote"]))
            flowables.append(Spacer(1, 2))
            continue

        # Tables: a line of | ... | followed by a separator line of |---|---|
        if line.lstrip().startswith("|") and i + 1 < n and re.match(
            r"^\s*\|[\s\-:|]+\|?\s*$", lines[i + 1]
        ):
            table_block = []
            # Header row
            table_block.append(parse_table_row(line))
            i += 1  # move to separator
            i += 1  # skip separator
            # Body rows
            while i < n and lines[i].lstrip().startswith("|"):
                table_block.append(parse_table_row(lines[i]))
                i += 1
            flowables.append(render_table(table_block, styles))
            flowables.append(Spacer(1, 3))
            continue

        # Numbered list items: "1. " "2. " etc
        if re.match(r"^\s*\d+\.\s+", line):
            items = []
            while i < n and re.match(r"^\s*\d+\.\s+", lines[i]):
                item_text = re.sub(r"^\s*\d+\.\s+", "", lines[i])
                items.append(Paragraph(render_inline(item_text), styles["li"]))
                i += 1
            flowables.append(ListFlowable(
                items,
                bulletType="1",
                leftIndent=14,
                bulletFontName=FONT_FAMILY_SANS,
                bulletFontSize=8.5,
                bulletColor=PALETTE["text_muted"],
            ))
            flowables.append(Spacer(1, 2))
            continue

        # Bulleted list items: "- " or "* "
        if re.match(r"^\s*[-*]\s+", line):
            items = []
            while i < n and re.match(r"^\s*[-*]\s+", lines[i]):
                item_text = re.sub(r"^\s*[-*]\s+", "", lines[i])
                items.append(ListItem(
                    Paragraph(render_inline(item_text), styles["li"]),
                    value="•",
                    bulletColor=PALETTE["accent"],
                ))
                i += 1
            flowables.append(ListFlowable(
                items,
                bulletType="bullet",
                leftIndent=14,
                bulletFontName=FONT_FAMILY_SANS,
                bulletFontSize=8.5,
            ))
            flowables.append(Spacer(1, 2))
            continue

        # Regular paragraph
        para_lines = [line]
        i += 1
        while i < n and lines[i].strip() and not (
            lines[i].lstrip().startswith(("#", ">", "|", "- ", "* "))
            or re.match(r"^\s*\d+\.\s+", lines[i])
            or lines[i].strip() in ("---", "***", "___")
        ):
            para_lines.append(lines[i])
            i += 1
        para_text = render_inline(" ".join(para_lines))
        flowables.append(Paragraph(para_text, styles["body"]))

    return flowables


def parse_table_row(line: str) -> list[str]:
    """Parse a markdown table row into cell strings."""
    # Strip leading/trailing pipes and whitespace
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|"):
        s = s[:-1]
    return [c.strip() for c in s.split("|")]


def is_group_row(row: list[str]) -> bool:
    """A table 'group row' is a full-width section band inside the table:
    the first cell is empty and the second cell is bold markdown."""
    return (
        len(row) >= 2
        and row[0] == ""
        and row[1].startswith("**") and row[1].endswith("**")
        and all(c == "" for c in row[2:])
    )


def render_table(rows: list[list[str]], styles: dict) -> Table:
    """Render a parsed markdown table as a ReportLab Table."""
    if not rows:
        return Spacer(1, 0)

    # Convert each cell to a Paragraph for proper wrapping
    para_rows = []
    group_row_indices = []
    for r_idx, row in enumerate(rows):
        if r_idx > 0 and is_group_row(row):
            group_row_indices.append(r_idx)
            para_row = [Paragraph(render_inline(row[1]), styles["table_group"])]
        else:
            para_row = []
            for cell in row:
                style = styles["table_header"] if r_idx == 0 else styles["table_cell"]
                para_row.append(Paragraph(render_inline(cell), style))
        # Normalize column count
        while len(para_row) < len(rows[0]):
            para_row.append(Paragraph("", styles["table_cell"]))
        para_rows.append(para_row)

    n_cols = max(len(row) for row in para_rows)

    # Content-aware column widths for the small column counts this
    # document uses. A 2-col table is label/value; a 3-col table is
    # index/name/description.
    if n_cols == 2:
        fractions = [0.30, 0.70]
    elif n_cols == 3:
        fractions = [0.08, 0.25, 0.67]
    else:
        fractions = [1.0 / n_cols] * n_cols
    col_widths = [CONTENT_WIDTH * f for f in fractions]

    table = Table(
        para_rows,
        colWidths=col_widths,
        repeatRows=1,  # repeat header on each page
        hAlign="LEFT",
    )

    style_cmds = [
        ("BACKGROUND", (0, 0), (-1, 0), PALETTE["table_header_bg"]),
        ("LINEBELOW", (0, 0), (-1, 0), 1, PALETTE["border"]),
        ("LINEBELOW", (0, 1), (-1, -1), 0.3, PALETTE["border"]),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 4),
        ("RIGHTPADDING", (0, 0), (-1, -1), 4),
        ("TOPPADDING", (0, 0), (-1, -1), 1.6),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 1.6),
    ]
    # Group rows: full-width band with a soft accent background
    for r in group_row_indices:
        style_cmds.append(("SPAN", (0, r), (-1, r)))
        style_cmds.append(("BACKGROUND", (0, r), (-1, r), PALETTE["accent_soft"]))
        style_cmds.append(("TOPPADDING", (0, r), (-1, r), 3))
        style_cmds.append(("BOTTOMPADDING", (0, r), (-1, r), 2.5))
    # Alternating row backgrounds (skip group rows so the band stays clean)
    stripe = 0
    for r_idx in range(1, len(para_rows)):
        if r_idx in group_row_indices:
            stripe = 0
            continue
        stripe += 1
        if stripe % 2 == 0:
            style_cmds.append(
                ("BACKGROUND", (0, r_idx), (-1, r_idx), PALETTE["table_alt_row"])
            )

    table.setStyle(TableStyle(style_cmds))
    return table


# ---------- Page decoration ----------

def page_decor(canvas, doc):
    """Draw the page top accent stripe and the footer on every page."""
    canvas.saveState()
    # Footer
    canvas.setFillColor(PALETTE["text_dim"])
    canvas.setFont(FONT_FAMILY_SANS, 7.5)
    canvas.drawString(
        MARGIN_LR, 8 * mm,
        "ScholarScribe User Guide · v2.2.1",
    )
    canvas.drawRightString(
        A4[0] - MARGIN_LR, 8 * mm,
        f"Page {doc.page}",
    )
    canvas.setStrokeColor(PALETTE["border"])
    canvas.setLineWidth(0.3)
    canvas.line(
        MARGIN_LR, 11.5 * mm,
        A4[0] - MARGIN_LR, 11.5 * mm,
    )
    # Subtle accent stripe at the very top of every page
    canvas.setFillColor(PALETTE["accent"])
    canvas.rect(0, A4[1] - 1.5 * mm, A4[0], 1.5 * mm, fill=1, stroke=0)
    canvas.restoreState()


class TitleBand(Flowable):
    """Thin accent rule used to close the title band at the top of page 1."""

    def __init__(self, width: float, thickness: float = 1.0):
        super().__init__()
        self.width_ = width
        self.thickness = thickness
        self.hAlign = "CENTER"

    def wrap(self, availWidth, availHeight):
        return self.width_, self.thickness + 2

    def draw(self):
        c = self.canv
        c.setStrokeColor(PALETTE["accent"])
        c.setLineWidth(self.thickness)
        c.line(0, self.thickness, self.width_, self.thickness)


def build_title_band(styles: dict, logo_path: Path | None = None) -> list:
    """Build the compact title band that opens page 1: logo and closing
    accent rule. The title itself and the pitch/version lines come from
    the markdown header (H1 + blockquotes), rendered by the parser.
    This replaces the old separate cover page so the whole document
    fits 2 pages."""
    flow = []

    if logo_path and logo_path.exists():
        try:
            # Render at 22mm wide, preserving aspect ratio (icons are 1:1).
            logo = Image(str(logo_path), width=22 * mm, height=22 * mm)
            logo.hAlign = "CENTER"
            flow.append(logo)
            flow.append(Spacer(1, 2.6 * mm))
        except Exception:
            # Image flowable can fail on bad PNG; non-fatal.
            pass

    flow.append(TitleBand(CONTENT_WIDTH * 0.32, thickness=1.1))
    flow.append(Spacer(1, 1.8 * mm))
    return flow


# ---------- Main ----------

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Generate the 2-page ScholarScribe User Guide PDF from USER_GUIDE.md.")
    parser.add_argument(
        "--input", default="USER_GUIDE.md",
        help="Path to USER_GUIDE.md (default: USER_GUIDE.md in cwd)",
    )
    parser.add_argument(
        "--output", default="ScholarScribe-User-Guide.pdf",
        help="Path to write the PDF (default: ScholarScribe-User-Guide.pdf in cwd)",
    )
    args = parser.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if not input_path.exists():
        print(f"ERROR: input file {input_path} does not exist", file=sys.stderr)
        return 1

    md_text = input_path.read_text(encoding="utf-8")

    # Split the header block (title + pitch + version + intro, up to the
    # first --- separator) from the body. The H1 and blockquote lines of
    # the header are re-rendered as the compact title band.
    sep_idx = md_text.find("\n---\n")
    if sep_idx == -1:
        header_md, body_md = md_text, ""
    else:
        header_md = md_text[:sep_idx]
        body_md = md_text[sep_idx + len("\n---\n"):]

    styles = build_styles()

    # Parse the header: the H1 becomes the title; the blockquote lines
    # become the centered pitch/version lines under it.
    header_flowables = parse_markdown_to_flowables(header_md, styles)

    body_flowables = parse_markdown_to_flowables(body_md, styles)

    # Build the document: single body template for all pages.
    doc = BaseDocTemplate(
        str(output_path),
        pagesize=A4,
        leftMargin=MARGIN_LR,
        rightMargin=MARGIN_LR,
        topMargin=MARGIN_TOP,
        bottomMargin=MARGIN_BOTTOM,
        title="ScholarScribe User Guide",
        author="Dr. Waleed Mandour",
        subject="ScholarScribe v2.2.1 User Guide",
        creator="ScholarScribe build pipeline",
    )

    frame = Frame(
        MARGIN_LR, MARGIN_BOTTOM,
        CONTENT_WIDTH, A4[1] - MARGIN_TOP - MARGIN_BOTTOM,
        leftPadding=0, rightPadding=0,
        topPadding=0, bottomPadding=0,
        id="main_frame",
    )
    doc.addPageTemplates([PageTemplate(id="main", frames=[frame], onPage=page_decor)])

    # Assemble the story: title band, then the header's pitch/version
    # lines, a rule, then the body.
    story = []
    logo_candidates = [
        Path(args.input).parent.parent / "src-tauri" / "icons" / "128x128@2x.png",
        Path(args.input).parent.parent / "src-tauri" / "icons" / "128x128.png",
        Path(args.input).parent.parent / "src-tauri" / "icons" / "icon.png",
    ]
    logo_path = next((p for p in logo_candidates if p.exists()), None)
    story.extend(build_title_band(styles, logo_path=logo_path))
    story.extend(header_flowables)
    story.append(HRFlowable(
        width="100%",
        thickness=0.5,
        color=PALETTE["border"],
        spaceBefore=3,
        spaceAfter=3,
    ))
    story.extend(body_flowables)

    doc.build(story)

    size_kb = output_path.stat().st_size / 1024
    print(f"Wrote {output_path} ({size_kb:.1f} KB)")

    # Page count (the guide is designed to fit exactly 2 pages)
    try:
        from pypdf import PdfReader
        reader = PdfReader(str(output_path))
        pages = len(reader.pages)
        print(f"Pages: {pages}")
        if pages != 2:
            print(f"WARNING: the guide is designed to fit exactly 2 pages, got {pages}.")
    except Exception as e:
        print(f"Could not count pages: {e}")


if __name__ == "__main__":
    sys.exit(main())
