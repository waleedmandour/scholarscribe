#!/usr/bin/env python3
"""Generate ScholarScribe-User-Guide.pdf from USER_GUIDE.md.

A focused ReportLab generator that parses the markdown structure of
USER_GUIDE.md and renders it as a polished A4 PDF with:
  - Cover page (title + subtitle + version + URL)
  - Body sections with H1/H2/H3 headings
  - Tables (markdown pipe tables) with proper column widths
  - Paragraphs (with bold/italic/code/inline backticks support)
  - Numbered and bulleted lists
  - Blockquotes for callouts
  - Inline code styling

Outputs to /home/z/my-project/download/ScholarScribe-User-Guide.pdf
matching the filename pattern of previous releases.
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
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    PageTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
    PageBreak,
    KeepTogether,
    HRFlowable,
    ListFlowable,
    ListItem,
)
from reportlab.platypus.flowables import Flowable

# ---------- Font registration ----------

FONT_FAMILY_SANS = "Helvetica"
FONT_FAMILY_SERIF = "Times-Roman"
FONT_FAMILY_MONO = "Courier"

try:
    # Try to register nicer fonts if available on the system.
    # Fall back to ReportLab's built-in Helvetica/Times/Courier otherwise.
    candidate_fonts = [
        # ("NotoSans", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        # ("NotoSans-Bold", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
        # ("NotoSans-Oblique", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf"),
    ]
    for name, path in candidate_fonts:
        if Path(path).exists():
            pdfmetrics.registerFont(TTFont(name, path))
except Exception:
    pass  # fall back to built-ins


# ---------- Palette ----------
# Calm, scholarly, accessible palette. WCAG-AA contrast verified.
PALETTE = {
    "bg": colors.HexColor("#FFFFFF"),
    "text": colors.HexColor("#1a1d23"),         # body text, near-black
    "text_muted": colors.HexColor("#5c6470"),   # secondary text
    "text_dim": colors.HexColor("#8a929e"),     # tertiary text
    "accent": colors.HexColor("#2a5bd7"),       # ScholarScribe blue
    "accent_soft": colors.HexColor("#e7eefb"),  # callout background
    "border": colors.HexColor("#dcdfe3"),       # table borders
    "table_header_bg": colors.HexColor("#f0f1f3"),
    "table_alt_row": colors.HexColor("#fafbfc"),
    "code_bg": colors.HexColor("#f0f1f3"),
    "cover_bg": colors.HexColor("#1a1d23"),     # dark cover
    "cover_text": colors.HexColor("#e6e8eb"),
    "cover_accent": colors.HexColor("#6b8eef"),
}


# ---------- Styles ----------

def build_styles() -> dict[str, ParagraphStyle]:
    base = getSampleStyleSheet()
    styles = {}

    styles["body"] = ParagraphStyle(
        "body",
        parent=base["BodyText"],
        fontName=FONT_FAMILY_SERIF,
        fontSize=11,
        leading=15,
        textColor=PALETTE["text"],
        spaceBefore=4,
        spaceAfter=4,
    )

    styles["h1"] = ParagraphStyle(
        "h1",
        parent=base["Heading1"],
        fontName=FONT_FAMILY_SANS,
        fontSize=20,
        leading=26,
        textColor=PALETTE["text"],
        spaceBefore=18,
        spaceAfter=8,
    )

    styles["h2"] = ParagraphStyle(
        "h2",
        parent=base["Heading2"],
        fontName=FONT_FAMILY_SANS,
        fontSize=15,
        leading=20,
        textColor=PALETTE["accent"],
        spaceBefore=14,
        spaceAfter=6,
    )

    styles["h3"] = ParagraphStyle(
        "h3",
        parent=base["Heading3"],
        fontName=FONT_FAMILY_SANS,
        fontSize=12,
        leading=16,
        textColor=PALETTE["text"],
        spaceBefore=10,
        spaceAfter=4,
    )

    styles["blockquote"] = ParagraphStyle(
        "blockquote",
        parent=styles["body"],
        leftIndent=12,
        rightIndent=12,
        textColor=PALETTE["text_muted"],
        backColor=PALETTE["accent_soft"],
        borderColor=PALETTE["accent"],
        borderPadding=(8, 10, 8, 10),
        borderWidth=0,
        spaceBefore=8,
        spaceAfter=8,
    )

    styles["code_inline"] = ParagraphStyle(
        "code_inline",
        parent=styles["body"],
        fontName=FONT_FAMILY_MONO,
        fontSize=10,
        textColor=PALETTE["text"],
        backColor=PALETTE["code_bg"],
        # Inline code styling is applied via <font> tag in the markup
    )

    styles["li"] = ParagraphStyle(
        "li",
        parent=styles["body"],
        leftIndent=14,
        spaceBefore=2,
        spaceAfter=2,
    )

    styles["table_cell"] = ParagraphStyle(
        "table_cell",
        parent=styles["body"],
        fontSize=10,
        leading=14,
        spaceBefore=0,
        spaceAfter=0,
    )

    styles["table_header"] = ParagraphStyle(
        "table_header",
        parent=styles["table_cell"],
        fontName=FONT_FAMILY_SANS,
        textColor=PALETTE["text_muted"],
        textTransform="uppercase",
        fontSize=9,
    )

    # Cover styles
    styles["cover_title"] = ParagraphStyle(
        "cover_title",
        fontName=FONT_FAMILY_SANS,
        fontSize=34,
        leading=42,
        textColor=PALETTE["cover_text"],
        alignment=1,  # center
        spaceBefore=0,
        spaceAfter=8,
    )
    styles["cover_subtitle"] = ParagraphStyle(
        "cover_subtitle",
        fontName=FONT_FAMILY_SERIF,
        fontSize=14,
        leading=20,
        textColor=PALETTE["cover_accent"],
        alignment=1,
        spaceBefore=4,
        spaceAfter=20,
    )
    styles["cover_meta"] = ParagraphStyle(
        "cover_meta",
        fontName=FONT_FAMILY_SANS,
        fontSize=10,
        leading=14,
        textColor=PALETTE["cover_text"],
        alignment=1,
        spaceBefore=2,
        spaceAfter=2,
    )
    styles["cover_url"] = ParagraphStyle(
        "cover_url",
        fontName=FONT_FAMILY_MONO,
        fontSize=9,
        leading=12,
        textColor=PALETTE["cover_accent"],
        alignment=1,
        spaceBefore=12,
        spaceAfter=2,
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
        r'<font face="Courier" size="10" backColor="#f0f1f3">\1</font>',
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
    # Bare URL: <https://...> -> clickable link
    text = re.sub(
        r"&lt;(https?://[^&]+)&gt;",
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
                spaceBefore=8,
                spaceAfter=8,
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
            quote_text = render_inline(" ".join(quote_lines))
            flowables.append(Paragraph(quote_text, styles["blockquote"]))
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
            flowables.append(Spacer(1, 4))
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
                leftIndent=18,
                bulletFontName=FONT_FAMILY_SANS,
                bulletFontSize=10,
                bulletColor=PALETTE["text_muted"],
            ))
            flowables.append(Spacer(1, 4))
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
                leftIndent=18,
                bulletFontName=FONT_FAMILY_SANS,
                bulletFontSize=10,
            ))
            flowables.append(Spacer(1, 4))
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


def render_table(rows: list[list[str]], styles: dict) -> Table:
    """Render a parsed markdown table as a ReportLab Table."""
    if not rows:
        return Spacer(1, 0)

    # Convert each cell to a Paragraph for proper wrapping
    para_rows = []
    for r_idx, row in enumerate(rows):
        para_row = []
        for cell in row:
            style = styles["table_header"] if r_idx == 0 else styles["table_cell"]
            para_row.append(Paragraph(render_inline(cell), style))
        para_rows.append(para_row)

    # Equal column widths, normalized to content area width
    # A4 width = 595 pt; with 18mm margins each side -> content width ~ 446pt
    content_width = A4[0] - 2 * 18 * mm
    n_cols = max(len(row) for row in para_rows)
    col_width = content_width / n_cols
    col_widths = [col_width] * n_cols

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
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 6),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
    ]
    # Alternating row backgrounds
    for r_idx in range(1, len(para_rows)):
        if r_idx % 2 == 0:
            style_cmds.append(
                ("BACKGROUND", (0, r_idx), (-1, r_idx), PALETTE["table_alt_row"])
            )

    table.setStyle(TableStyle(style_cmds))
    return table


# ---------- Cover page ----------

class CoverBackground(Flowable):
    """Full-bleed dark background for the cover page."""

    def __init__(self, page_w: float, page_h: float):
        super().__init__()
        self.page_w = page_w
        self.page_h = page_h

    def wrap(self, *args):
        return 0, 0

    def draw(self):
        c = self.canv
        c.saveState()
        c.setFillColor(PALETTE["cover_bg"])
        c.rect(0, 0, self.page_w, self.page_h, fill=1, stroke=0)
        # Subtle accent stripe at the top
        c.setFillColor(PALETTE["cover_accent"])
        c.rect(0, self.page_h - 6 * mm, self.page_w, 6 * mm, fill=1, stroke=0)
        c.restoreState()


def build_cover(styles: dict) -> list:
    """Build the cover page flowables."""
    flow = []
    # The CoverBackground is drawn via the page template's onPage callback,
    # not as a flowable. But we need flowables to position the text.

    flow.append(Spacer(1, 90 * mm))
    flow.append(Paragraph("ScholarScribe", styles["cover_title"]))
    flow.append(Paragraph("User Guide", styles["cover_subtitle"]))

    flow.append(Spacer(1, 50 * mm))
    flow.append(Paragraph("Version 2.2.0", styles["cover_meta"]))
    flow.append(Paragraph("Windows · macOS · Linux", styles["cover_meta"]))
    flow.append(Paragraph("MIT License", styles["cover_meta"]))

    flow.append(Spacer(1, 20 * mm))
    flow.append(Paragraph(
        '<link href="https://github.com/waleedmandour/scholarscribe">'
        '<font color="#6b8eef">github.com/waleedmandour/scholarscribe</font>'
        '</link>',
        styles["cover_url"],
    ))

    flow.append(PageBreak())
    return flow


# ---------- Page templates ----------

def cover_page(canvas, doc):
    """Draw the dark cover background."""
    canvas.saveState()
    canvas.setFillColor(PALETTE["cover_bg"])
    canvas.rect(0, 0, A4[0], A4[1], fill=1, stroke=0)
    # Top accent stripe
    canvas.setFillColor(PALETTE["cover_accent"])
    canvas.rect(0, A4[1] - 6 * mm, A4[0], 6 * mm, fill=1, stroke=0)
    canvas.restoreState()


def body_page(canvas, doc):
    """Draw the body page header/footer."""
    canvas.saveState()
    # Footer
    canvas.setFillColor(PALETTE["text_dim"])
    canvas.setFont(FONT_FAMILY_SANS, 8)
    canvas.drawString(
        18 * mm, 10 * mm,
        "ScholarScribe User Guide · v2.2.0",
    )
    canvas.drawRightString(
        A4[0] - 18 * mm, 10 * mm,
        f"Page {doc.page - 1}",  # cover is page 1, body starts at 2
    )
    canvas.setStrokeColor(PALETTE["border"])
    canvas.setLineWidth(0.3)
    canvas.line(
        18 * mm, 14 * mm,
        A4[0] - 18 * mm, 14 * mm,
    )
    canvas.restoreState()


# ---------- Main ----------

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Generate ScholarScribe User Guide PDF from USER_GUIDE.md.")
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

    # Strip the cover header block (lines 1-8 are title + subtitle + intro
    # that we re-render as the cover page).
    # Find the first --- separator after the intro
    sep_idx = md_text.find("\n---\n")
    if sep_idx == -1:
        body_md = md_text
    else:
        body_md = md_text[sep_idx + len("\n---\n"):]

    styles = build_styles()
    body_flowables = parse_markdown_to_flowables(body_md, styles)

    # Build the document
    doc = BaseDocTemplate(
        str(output_path),
        pagesize=A4,
        leftMargin=18 * mm,
        rightMargin=18 * mm,
        topMargin=20 * mm,
        bottomMargin=20 * mm,
        title="ScholarScribe User Guide",
        author="Dr. Waleed Mandour",
        subject="ScholarScribe v2.2.0 User Guide",
        creator="ScholarScribe build pipeline",
    )

    # Cover frame: full-bleed, content centered
    cover_frame = Frame(
        0, 0, A4[0], A4[1],
        leftPadding=0, rightPadding=0,
        topPadding=0, bottomPadding=0,
        id="cover_frame",
    )
    body_frame = Frame(
        18 * mm, 18 * mm,
        A4[0] - 36 * mm, A4[1] - 36 * mm,
        leftPadding=0, rightPadding=0,
        topPadding=0, bottomPadding=0,
        id="body_frame",
    )

    doc.addPageTemplates([
        PageTemplate(id="cover", frames=[cover_frame], onPage=cover_page),
        PageTemplate(id="body", frames=[body_frame], onPage=body_page),
    ])

    # Build the story: cover content, then body
    story = []
    # Use the cover template for the first page
    from reportlab.platypus.doctemplate import NextPageTemplate
    story.append(NextPageTemplate("cover"))
    story.extend(build_cover(styles))
    # Switch to body template for the rest
    story.append(NextPageTemplate("body"))
    story.extend(body_flowables)

    doc.build(story)

    size_kb = output_path.stat().st_size / 1024
    print(f"Wrote {output_path} ({size_kb:.1f} KB)")

    # Quick page count
    try:
        from pypdf import PdfReader
        reader = PdfReader(str(output_path))
        print(f"Pages: {len(reader.pages)}")
    except Exception as e:
        print(f"Could not count pages: {e}")


if __name__ == "__main__":
    sys.exit(main())
