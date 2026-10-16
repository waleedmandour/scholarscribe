//! .docx → LaTeX conversion (v2.2.1 r3; v2.2.2 r1 footnote/endnote extraction).
//!
//! Converts a Word document into a compilable LaTeX article through a
//! deterministic, rule-based structural mapping. No network, no LLM: this
//! is a local structural transformation of the document's WordprocessingML,
//! consistent with the project's design of auditable, reproducible text
//! tools (docs/ETHICS.md).
//!
//! Mapping (v1, deliberately small and predictable):
//! - `Title` style              → `\title{...}` + `\maketitle`
//! - `Heading1`                 → `\section{...}`
//! - `Heading2`                 → `\subsection{...}`
//! - `Heading3`                 → `\subsubsection{...}`
//! - `Heading4`-`Heading6`     → `\paragraph{...}`
//! - list paragraphs (`w:numPr`) → `\begin{itemize}` / `\item` runs
//! - tables (`w:tbl`)           → simple `\begin{tabular}` grids
//! - bold / italic runs         → `\textbf{...}` / `\emph{...}`
//! - `w:br` inside a paragraph  → LaTeX line break `\\`
//! - everything else            → plain paragraphs, LaTeX-escaped
//!
//! Unknown paragraph styles are passed through as body text and reported in
//! `warnings` so the user knows exactly what the converter did not map.
//! Merged table cells (`gridSpan`/`vMerge`) and nested tables are flattened
//! and counted in `warnings`.
//!
//! v2.2.2 r1: previously only `word/document.xml` was read, so references
//! stored in footnotes (`word/footnotes.xml`) or endnotes
//! (`word/endnotes.xml`) — common in Chicago notes-bibliography,
//! Vancouver-style, and any manuscript using Word's "Insert Footnote" for
//! citations — were silently dropped, contradicting the "never silently
//! dropped" promise. The converter now reads all three parts; footnote and
//! endnote paragraphs are parsed with the same `parse_blocks` walker and
//! appended under `\section*{Footnotes}` / `\section*{Endnotes}` headings
//! at the end of the body. A warning is recorded per part so the user can
//! verify the extraction against the source document.

use std::io::Read;
use std::path::Path;

/// Outcome of a successful conversion, surfaced in the UI so the user can
/// verify the mapping against the source document.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DocxToLatexResult {
    pub output_path: String,
    pub paragraphs: usize,
    pub headings: usize,
    pub list_items: usize,
    pub tables: usize,
    pub warnings: Vec<String>,
}

/// Convert a .docx file into a LaTeX article saved at `output_path`.
///
/// Reads `word/document.xml` plus, when present, `word/footnotes.xml` and
/// `word/endnotes.xml` (v2.2.2 r1). Each part is parsed with the same
/// `parse_blocks` walker; footnote/endnote blocks are appended under
/// `\section*{Footnotes}` / `\section*{Endnotes}` headings so no
/// user-visible text is silently dropped.
pub fn convert_docx_to_latex(path: &Path, output_path: &Path) -> Result<DocxToLatexResult, String> {
    // Read the .docx as a zip once; reuse the archive for every part so
    // footnotes/endnotes extraction is essentially free.
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("not a readable .docx ({e})"))?;

    let document_xml = read_zip_part(&mut archive, "word/document.xml")?.ok_or_else(|| {
        "word/document.xml not found in .docx; the file may be corrupt or not a real .docx"
            .to_string()
    })?;

    let (mut blocks, mut parse_info) = parse_blocks(&document_xml)?;
    let mut footnote_paragraphs = 0usize;
    let mut endnote_paragraphs = 0usize;

    // Block-index boundary where the footnote section begins (if any) and
    // where the endnote section begins (if any). `None` when the part is
    // absent or empty. `render` uses these to inject `\section*{Footnotes}`
    // / `\section*{Endnotes}` headings at the exact position.
    let mut footnote_start: Option<usize> = None;
    let mut endnote_start: Option<usize> = None;

    // Footnotes: many academic styles (Chicago notes-bibliography, Vancouver
    // with footnote citations, any Word "Insert Footnote" reference list)
    // store the bibliography here. Without this extraction the user would
    // see only the body-text references converted and conclude the converter
    // was truncating their reference list.
    if let Some(footnote_xml) = read_zip_part(&mut archive, "word/footnotes.xml")? {
        let (fn_blocks, fn_info) = parse_blocks(&footnote_xml)?;
        if !fn_blocks.is_empty() {
            footnote_start = Some(blocks.len());
            footnote_paragraphs = fn_blocks
                .iter()
                .filter(|b| {
                    matches!(b, Block::Para(p) if !runs_to_latex(&p.runs).trim().is_empty())
                })
                .count();
            parse_info.merged_cells += fn_info.merged_cells;
            parse_info.nested_tables += fn_info.nested_tables;
            blocks.extend(fn_blocks);
        }
    }

    if let Some(endnote_xml) = read_zip_part(&mut archive, "word/endnotes.xml")? {
        let (en_blocks, en_info) = parse_blocks(&endnote_xml)?;
        if !en_blocks.is_empty() {
            endnote_start = Some(blocks.len());
            endnote_paragraphs = en_blocks
                .iter()
                .filter(|b| {
                    matches!(b, Block::Para(p) if !runs_to_latex(&p.runs).trim().is_empty())
                })
                .count();
            parse_info.merged_cells += en_info.merged_cells;
            parse_info.nested_tables += en_info.nested_tables;
            blocks.extend(en_blocks);
        }
    }

    let rendered = render(
        &blocks,
        &parse_info,
        footnote_paragraphs,
        endnote_paragraphs,
        footnote_start,
        endnote_start,
    );

    std::fs::write(output_path, &rendered.latex)
        .map_err(|e| format!("write {}: {e}", output_path.display()))?;

    Ok(DocxToLatexResult {
        output_path: output_path.display().to_string(),
        paragraphs: rendered.counts.paragraphs,
        headings: rendered.counts.headings,
        list_items: rendered.counts.list_items,
        tables: rendered.counts.tables,
        warnings: rendered.warnings,
    })
}

/// Read a single named part out of a .docx zip archive. Returns
/// `Ok(Some(content))` when the part is present, `Ok(None)` when the .docx
/// is valid but does not carry that part (footnotes/endnotes are optional),
/// and `Err` for zip/read errors. Reused for `word/document.xml`,
/// `word/footnotes.xml`, and `word/endnotes.xml`.
fn read_zip_part<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    part_name: &str,
) -> Result<Option<String>, String> {
    let mut buf = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        if entry.name() == part_name {
            buf.clear();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("read {part_name}: {e}"))?;
            return Ok(Some(String::from_utf8_lossy(&buf).into_owned()));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Block model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
}

#[derive(Debug, Clone)]
struct Para {
    style: Option<String>,
    in_list: bool,
    runs: Vec<Run>,
}

#[derive(Debug, Clone)]
struct Table {
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
enum Block {
    Para(Para),
    Table(Table),
}

#[derive(Debug, Default)]
struct ParseInfo {
    merged_cells: usize,
    nested_tables: usize,
}

// ---------------------------------------------------------------------------
// Parsing (quick-xml walk, same pattern as docx_reading::parse_track_changes_ooxml)
// ---------------------------------------------------------------------------

fn parse_blocks(xml: &str) -> Result<(Vec<Block>, ParseInfo), String> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut info = ParseInfo::default();
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();

    // Paragraph under construction (outside tables).
    let mut cur: Option<Para> = None;
    let mut run_text = String::new();
    let mut run_bold = false;
    let mut run_italic = false;
    let mut in_wt = false; // inside <w:t>
    let mut in_ppr = false; // inside <w:pPr>
    let mut in_rpr = false; // inside <w:rPr>
    let mut para_active = false;

    // Table state. Only depth-1 tables become tabular blocks; nested tables
    // are flattened into the enclosing cell (counted in ParseInfo).
    let mut table_depth = 0usize;
    let mut cell_text = String::new();
    let mut row: Vec<String> = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => match e.name().as_ref() {
                b"w:p" => {
                    para_active = true;
                    if table_depth == 0 {
                        cur = Some(Para {
                            style: None,
                            in_list: false,
                            runs: Vec::new(),
                        });
                    }
                }
                b"w:pPr" => in_ppr = true,
                b"w:pStyle" if in_ppr && table_depth == 0 => {
                    if let Some(p) = cur.as_mut() {
                        p.style = attr_val(&e, b"w:val");
                    }
                }
                b"w:numPr" if in_ppr && table_depth == 0 => {
                    if let Some(p) = cur.as_mut() {
                        p.in_list = true;
                    }
                }
                b"w:r" => {
                    run_text.clear();
                    run_bold = false;
                    run_italic = false;
                }
                b"w:rPr" => in_rpr = true,
                b"w:b" if in_rpr => run_bold = flag_on(&e),
                b"w:i" if in_rpr => run_italic = flag_on(&e),
                b"w:t" => in_wt = true,
                b"w:tbl" => {
                    if table_depth == 0 {
                        blocks.push(Block::Table(Table { rows: Vec::new() }));
                    } else {
                        info.nested_tables += 1;
                    }
                    table_depth += 1;
                }
                b"w:tr" if table_depth == 1 => row = Vec::new(),
                b"w:tc" if table_depth == 1 => cell_text = String::new(),
                b"w:gridSpan" | b"w:vMerge" if table_depth >= 1 => info.merged_cells += 1,
                _ => {}
            },
            Ok(quick_xml::events::Event::Empty(e)) => match e.name().as_ref() {
                b"w:pStyle" if in_ppr && table_depth == 0 => {
                    if let Some(p) = cur.as_mut() {
                        p.style = attr_val(&e, b"w:val");
                    }
                }
                b"w:numPr" if in_ppr && table_depth == 0 => {
                    if let Some(p) = cur.as_mut() {
                        p.in_list = true;
                    }
                }
                b"w:b" if in_rpr => run_bold = flag_on(&e),
                b"w:i" if in_rpr => run_italic = flag_on(&e),
                b"w:tab" if in_wt => run_text.push(' '),
                b"w:br" if in_wt => run_text.push('\n'),
                b"w:gridSpan" | b"w:vMerge" if table_depth >= 1 => info.merged_cells += 1,
                _ => {}
            },
            Ok(quick_xml::events::Event::Text(t)) => {
                if in_wt {
                    if let Ok(txt) = t.unescape() {
                        run_text.push_str(&txt);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(e)) => match e.name().as_ref() {
                b"w:t" => in_wt = false,
                b"w:rPr" => in_rpr = false,
                b"w:pPr" => in_ppr = false,
                b"w:r" => {
                    if !run_text.is_empty() {
                        if table_depth == 0 {
                            if let Some(p) = cur.as_mut() {
                                p.runs.push(Run {
                                    text: run_text.clone(),
                                    bold: run_bold,
                                    italic: run_italic,
                                });
                            }
                        } else if table_depth == 1 {
                            if !cell_text.is_empty() {
                                cell_text.push(' ');
                            }
                            cell_text.push_str(&run_text);
                        } else {
                            // Nested-table content lands in the enclosing cell.
                            if !cell_text.is_empty() {
                                cell_text.push(' ');
                            }
                            cell_text.push_str(&run_text);
                        }
                    }
                    run_text.clear();
                }
                b"w:p" => {
                    if table_depth == 0 && para_active {
                        if let Some(p) = cur.take() {
                            blocks.push(Block::Para(p));
                        }
                    }
                    para_active = false;
                }
                b"w:tc" if table_depth == 1 => row.push(std::mem::take(&mut cell_text)),
                b"w:tr" if table_depth == 1 => {
                    if let Some(Block::Table(t)) = blocks.last_mut() {
                        t.rows.push(std::mem::take(&mut row));
                    }
                }
                b"w:tbl" => table_depth = table_depth.saturating_sub(1),
                _ => {}
            },
            Ok(quick_xml::events::Event::Eof) => break,
            Err(err) => return Err(format!("XML parse failure: {err}")),
            _ => {}
        }
    }

    // Unterminated paragraph at EOF: flush if we can.
    if table_depth == 0 && para_active {
        if let Some(p) = cur.take() {
            blocks.push(Block::Para(p));
        }
    }
    Ok((blocks, info))
}

/// Read an attribute value off a start tag (raw qualified-name match, like
/// docx_reading::attr_value), XML-unescaping entities.
fn attr_val(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    for attr in e.attributes() {
        let attr = attr.ok()?;
        if attr.key.as_ref() == name {
            let raw = String::from_utf8_lossy(&attr.value).into_owned();
            return Some(
                raw.replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'")
                    .replace("&#39;", "'")
                    .replace("&amp;", "&"),
            );
        }
    }
    None
}

/// `w:val` flag semantics: absent or "1"/"true"/"on" → on; "0"/"false"/"off" → off.
fn flag_on(e: &quick_xml::events::BytesStart<'_>) -> bool {
    match attr_val(e, b"w:val") {
        None => true,
        Some(v) => !matches!(v.as_str(), "0" | "false" | "off"),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    paragraphs: usize,
    headings: usize,
    list_items: usize,
    tables: usize,
}

#[derive(Debug)]
struct Rendered {
    latex: String,
    counts: Counts,
    warnings: Vec<String>,
}

fn escape_tex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' => out.push_str("\\&"),
            '%' => out.push_str("\\%"),
            '$' => out.push_str("\\$"),
            '#' => out.push_str("\\#"),
            '_' => out.push_str("\\_"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\u{00A0}' | '\u{2007}' | '\u{202F}' => out.push('~'),
            _ => out.push(c),
        }
    }
    out
}

/// Render the parsed blocks as a complete LaTeX document.
///
/// `footnote_paragraphs` / `endnote_paragraphs` are the non-empty
/// paragraph counts extracted from `word/footnotes.xml` and
/// `word/endnotes.xml`. `footnote_start` / `endnote_start` are the
/// block indices where each section begins (None when the part was
/// absent or empty). When non-`None`, the render injects
/// `\section*{Footnotes}` / `\section*{Endnotes}` headings immediately
/// before the first block at that index so the user can see where each
/// part of the source document landed in the LaTeX output. The injected
/// headings are not counted in `counts.headings` (they are structural
/// dividers for extracted content, not source headings).
fn render(
    blocks: &[Block],
    info: &ParseInfo,
    footnote_paragraphs: usize,
    endnote_paragraphs: usize,
    footnote_start: Option<usize>,
    endnote_start: Option<usize>,
) -> Rendered {
    let mut warnings: Vec<String> = Vec::new();
    if info.merged_cells > 0 {
        warnings.push(format!(
            "{} merged table cell(s) (gridSpan/vMerge) were flattened to plain cells.",
            info.merged_cells
        ));
    }
    if info.nested_tables > 0 {
        warnings.push(format!(
            "{} nested table(s) were flattened into the enclosing cell.",
            info.nested_tables
        ));
    }
    if footnote_paragraphs > 0 {
        warnings.push(format!(
            "{footnote_paragraphs} footnote paragraph(s) were extracted from word/footnotes.xml and appended under a \"Footnotes\" section. Verify against the source document.",
        ));
    }
    if endnote_paragraphs > 0 {
        warnings.push(format!(
            "{endnote_paragraphs} endnote paragraph(s) were extracted from word/endnotes.xml and appended under an \"Endnotes\" section. Verify against the source document.",
        ));
    }

    let mut unknown_styles: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut title: Option<String> = None;
    let mut counts = Counts::default();

    let mut i = 0usize;
    while i < blocks.len() {
        // Inject the Footnotes section heading before the first footnote block.
        if let Some(start) = footnote_start {
            if i == start {
                body.push_str("\\section*{Footnotes}\n\n");
            }
        }
        // Inject the Endnotes section heading before the first endnote block.
        if let Some(start) = endnote_start {
            if i == start {
                body.push_str("\\section*{Endnotes}\n\n");
            }
        }

        match &blocks[i] {
            Block::Para(p) => {
                let text = runs_to_latex(&p.runs);
                if text.trim().is_empty() {
                    i += 1;
                    continue;
                }
                let style = p.style.as_deref().unwrap_or("");
                match style {
                    "Title" => {
                        if title.is_none() {
                            title = Some(text);
                        } else {
                            body.push_str(&format!("\\section*{{{text}}}\n\n"));
                        }
                    }
                    "Heading1" | "heading 1" => {
                        counts.headings += 1;
                        body.push_str(&format!("\\section{{{text}}}\n\n"));
                    }
                    "Heading2" | "heading 2" => {
                        counts.headings += 1;
                        body.push_str(&format!("\\subsection{{{text}}}\n\n"));
                    }
                    "Heading3" | "heading 3" => {
                        counts.headings += 1;
                        body.push_str(&format!("\\subsubsection{{{text}}}\n\n"));
                    }
                    "Heading4" | "Heading5" | "Heading6" | "heading 4" | "heading 5"
                    | "heading 6" => {
                        counts.headings += 1;
                        body.push_str(&format!("\\paragraph{{{text}}}\n\n"));
                    }
                    _ if p.in_list => {
                        // Collect the full consecutive run of list paragraphs,
                        // then emit one itemize environment.
                        counts.list_items += 1;
                        let mut items = format!("  \\item {text}\n");
                        let mut j = i + 1;
                        while j < blocks.len() {
                            if let Block::Para(next) = &blocks[j] {
                                if next.in_list {
                                    let t = runs_to_latex(&next.runs);
                                    if !t.trim().is_empty() {
                                        counts.list_items += 1;
                                        items.push_str(&format!("  \\item {t}\n"));
                                    }
                                    j += 1;
                                    continue;
                                }
                            }
                            break;
                        }
                        body.push_str("\\begin{itemize}\n");
                        body.push_str(&items);
                        body.push_str("\\end{itemize}\n\n");
                        i = j;
                        continue;
                    }
                    _ => {
                        if !style.is_empty() && !unknown_styles.iter().any(|s| s == style) {
                            unknown_styles.push(style.to_string());
                        }
                        counts.paragraphs += 1;
                        body.push_str(&format!("{text}\n\n"));
                    }
                }
                i += 1;
            }
            Block::Table(t) => {
                counts.tables += 1;
                body.push_str(&render_table(t));
                i += 1;
            }
        }
    }

    if !unknown_styles.is_empty() {
        warnings.push(format!(
            "Paragraph style(s) with no LaTeX mapping were emitted as plain body text: {}.",
            unknown_styles.join(", ")
        ));
    }

    let mut latex = String::new();
    latex.push_str(
        "% Converted from .docx by ScholarScribe (deterministic structural conversion,\n% no network, no LLM). Compile with pdflatex or xelatex.\n",
    );
    latex.push_str("\\documentclass[11pt,a4paper]{article}\n");
    latex.push_str("\\usepackage[utf8]{inputenc}\n");
    latex.push_str("\\usepackage[T1]{fontenc}\n");
    latex.push_str("\\begin{document}\n\n");
    if let Some(t) = title {
        latex.push_str(&format!("\\title{{{t}}}\n\\maketitle\n\n"));
    }
    latex.push_str(&body);
    latex.push_str("\\end{document}\n");

    Rendered {
        latex,
        counts,
        warnings,
    }
}

/// Convert runs to LaTeX text, merging consecutive runs with identical
/// formatting so `\textbf{...}` spans stay readable.
fn runs_to_latex(runs: &[Run]) -> String {
    let mut out = String::new();
    let mut idx = 0usize;
    while idx < runs.len() {
        let bold = runs[idx].bold;
        let italic = runs[idx].italic;
        let mut seg = String::new();
        while idx < runs.len() && runs[idx].bold == bold && runs[idx].italic == italic {
            seg.push_str(&runs[idx].text);
            idx += 1;
        }
        let seg = escape_tex(&seg);
        let seg = seg.replace('\n', " \\\\\n").replace('\t', " ");
        if bold && italic {
            out.push_str(&format!("\\textbf{{\\emph{{{seg}}}}}"));
        } else if bold {
            out.push_str(&format!("\\textbf{{{seg}}}"));
        } else if italic {
            out.push_str(&format!("\\emph{{{seg}}}"));
        } else {
            out.push_str(&seg);
        }
    }
    out
}

fn render_table(t: &Table) -> String {
    let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("\\begin{tabular}{|");
    for _ in 0..cols {
        out.push('l');
        out.push('|');
    }
    out.push_str("}\n\\hline\n");
    for row in &t.rows {
        let mut cells: Vec<String> = Vec::with_capacity(cols);
        for c in 0..cols {
            let cell = row.get(c).map(|s| escape_tex(s)).unwrap_or_default();
            cells.push(cell.replace('\n', " "));
        }
        out.push_str(&cells.join(" & "));
        out.push_str(" \\\\\n\\hline\n");
    }
    out.push_str("\\end{tabular}\n\n");
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal in-memory .docx with the given document.xml body.
    fn write_test_docx(dir: &Path, name: &str, document_xml: &str) -> std::path::PathBuf {
        let file_path = dir.join(name);
        let f = std::fs::File::create(&file_path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("word/document.xml", opts).unwrap();
        std::io::Write::write_all(&mut zip, document_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
        file_path
    }

    /// Build an in-memory .docx with `word/document.xml` plus optional
    /// `word/footnotes.xml` and `word/endnotes.xml` parts. The extra parts
    /// are added only when the corresponding `&str` is `Some(...)`, mirroring
    /// the structure MS Word actually produces for academic manuscripts
    /// whose references live in footnotes (Chicago notes-bibliography,
    /// Vancouver with footnote citations, etc.).
    fn write_test_docx_with_parts(
        dir: &Path,
        name: &str,
        document_xml: &str,
        footnotes_xml: Option<&str>,
        endnotes_xml: Option<&str>,
    ) -> std::path::PathBuf {
        let file_path = dir.join(name);
        let f = std::fs::File::create(&file_path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("word/document.xml", opts).unwrap();
        std::io::Write::write_all(&mut zip, document_xml.as_bytes()).unwrap();
        if let Some(xml) = footnotes_xml {
            zip.start_file("word/footnotes.xml", opts).unwrap();
            std::io::Write::write_all(&mut zip, xml.as_bytes()).unwrap();
        }
        if let Some(xml) = endnotes_xml {
            zip.start_file("word/endnotes.xml", opts).unwrap();
            std::io::Write::write_all(&mut zip, xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        file_path
    }

    /// Build document.xml with `n` plain-paragraph references under a
    /// "References" Heading1. This is the most common real-world reference
    /// layout: each reference is its own `<w:p>` with no numPr and no
    /// paragraph style. The bug report "only converts 5 references" can only
    /// be reproduced by this test if the parser has a hidden cap — which it
    /// does not. The test exists to lock that property in.
    fn build_doc_xml_with_n_plain_references(n: usize) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        s.push_str("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n");
        s.push_str("<w:body>\n");
        s.push_str("<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>My Paper</w:t></w:r></w:p>\n");
        s.push_str(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>References</w:t></w:r></w:p>\n",
        );
        for i in 1..=n {
            s.push_str(&format!(
                "<w:p><w:r><w:t>Author{0}, A. (2020). Title number {0}. Journal of Tests, {0}({0}), 1-{0}.</w:t></w:r></w:p>\n",
                i
            ));
        }
        s.push_str("</w:body>\n</w:document>\n");
        s
    }

    /// Same as above but each reference is a numbered-list paragraph
    /// (`<w:numPr>` with the realistic `<w:ilvl>` + `<w:numId>` children
    /// that MS Word actually emits, not the self-closing `<w:numPr/>`
    /// shorthand used in the existing fixture). All n references should
    /// be collected into a single `itemize` environment.
    fn build_doc_xml_with_n_numbered_references(n: usize) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        s.push_str("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n");
        s.push_str("<w:body>\n");
        s.push_str("<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>My Paper</w:t></w:r></w:p>\n");
        s.push_str(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>References</w:t></w:r></w:p>\n",
        );
        for i in 1..=n {
            s.push_str(&format!(
                "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr></w:pPr><w:r><w:t>Author{0}, A. (2020). Title number {0}. Journal of Tests, {0}({0}), 1-{0}.</w:t></w:r></w:p>\n",
                i
            ));
        }
        s.push_str("</w:body>\n</w:document>\n");
        s
    }

    /// Build a footnotes.xml with `n` non-separator footnotes. Word always
    /// emits two boilerplate separator footnotes (w:id="-1" type="separator"
    /// and w:id="0" type="continuationSeparator") before the real content;
    /// we include them so the test mirrors a real .docx exactly. The
    /// separator paragraphs are empty `<w:p/>` so the converter filters
    /// them out via the `text.trim().is_empty()` check in `render`.
    fn build_footnotes_xml_with_n_references(n: usize) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        s.push_str("<w:footnotes xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n");
        // Separator boilerplate; both are empty paragraphs.
        s.push_str("<w:footnote w:id=\"-1\" w:type=\"separator\"><w:p/></w:footnote>\n");
        s.push_str(
            "<w:footnote w:id=\"0\" w:type=\"continuationSeparator\"><w:p/></w:footnote>\n",
        );
        for i in 1..=n {
            s.push_str(&format!(
                "<w:footnote w:id=\"{id}\"><w:p><w:r><w:t>Footnote reference {id}: Author{id}, B. (2021). Title number {id}. Footnote Journal, {id}({id}), 1-{id}.</w:t></w:r></w:p></w:footnote>\n",
                id = i
            ));
        }
        s.push_str("</w:footnotes>\n");
        s
    }

    fn build_endnotes_xml_with_n_references(n: usize) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        s.push_str("<w:endnotes xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n");
        s.push_str("<w:endnote w:id=\"-1\" w:type=\"separator\"><w:p/></w:endnote>\n");
        s.push_str(
            "<w:endnote w:id=\"0\" w:type=\"continuationSeparator\"><w:p/></w:endnote>\n",
        );
        for i in 1..=n {
            s.push_str(&format!(
                "<w:endnote w:id=\"{id}\"><w:p><w:r><w:t>Endnote reference {id}: Author{id}, C. (2022). Title number {id}. Endnote Journal, {id}({id}), 1-{id}.</w:t></w:r></w:p></w:endnote>\n",
                id = i
            ));
        }
        s.push_str("</w:endnotes>\n");
        s
    }

    #[test]
    fn escapes_latex_specials() {
        assert_eq!(
            escape_tex("100% & $5 _x_ {y}"),
            "100\\% \\& \\$5 \\_x\\_ \\{y\\}"
        );
        assert_eq!(
            escape_tex("a~b^c"),
            "a\\textasciitilde{}b\\textasciicircum{}c"
        );
        assert_eq!(escape_tex("back\\slash"), "back\\textbackslash{}slash");
    }

    #[test]
    fn converts_headings_bold_list_and_table() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>My Paper</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Introduction</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Bold claim:</w:t></w:r><w:r><w:t> 100% sure &amp; more</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>first item</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>second item</w:t></w:r></w:p>
<w:tbl>
<w:tr><w:tc><w:p><w:r><w:t>trial</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>score</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>A &amp; B</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>93</w:t></w:r></w:p></w:tc></w:tr>
</w:tbl>
</w:body>
</w:document>"#;

        let dir =
            std::env::temp_dir().join(format!("scholarscribe-latex-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "sample.docx", xml);
        let out = dir.join("sample.tex");

        let res = convert_docx_to_latex(&src, &out).expect("conversion should succeed");
        let latex = std::fs::read_to_string(&out).unwrap();

        assert!(latex.starts_with("% Converted from .docx by ScholarScribe"));
        assert!(latex.contains("\\documentclass[11pt,a4paper]{article}"));
        assert!(latex.contains("\\title{My Paper}"));
        assert!(latex.contains("\\maketitle"));
        assert!(latex.contains("\\section{Introduction}"));
        assert!(latex.contains("\\textbf{Bold claim:} 100\\% sure \\& more"));
        assert!(latex.contains("\\begin{itemize}"));
        assert!(latex.contains("\\item first item"));
        assert!(latex.contains("\\item second item"));
        assert!(latex.contains("\\end{itemize}"));
        assert!(latex.contains("\\begin{tabular}{|l|l|}"));
        assert!(latex.contains("A \\& B & 93"));
        assert!(latex.trim_end().ends_with("\\end{document}"));
        assert_eq!(res.headings, 1);
        assert_eq!(res.list_items, 2);
        assert_eq!(res.tables, 1);
        assert_eq!(res.output_path, out.display().to_string());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_unknown_styles_as_warning() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:pPr><w:pStyle w:val="TOC1"/></w:pPr><w:r><w:t>contents line</w:t></w:r></w:p>
</w:body></w:document>"#;
        let dir =
            std::env::temp_dir().join(format!("scholarscribe-latex-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "toc.docx", xml);
        let out = dir.join("toc.tex");
        let res = convert_docx_to_latex(&src, &out).unwrap();
        let latex = std::fs::read_to_string(&out).unwrap();
        assert!(latex.contains("contents line"));
        assert!(res.warnings.iter().any(|w| w.contains("TOC1")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_non_docx_files() {
        let dir =
            std::env::temp_dir().join(format!("scholarscribe-latex-test3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("not-a-docx.docx");
        std::fs::write(&src, b"plain text, not a zip").unwrap();
        let out = dir.join("out.tex");
        assert!(convert_docx_to_latex(&src, &out).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Regression test for the "only 5 references converted" bug report.
    ///
    /// The user reported that the Word-to-LaTeX converter only converts 5 of
    /// their references. The previous AI guessed at a `.take(5)` cap that
    /// does not exist in the source. This test, plus its numbered-list
    /// sibling below, locks in the actual behavior: every reference in the
    /// body is converted, no matter how many there are. 10 references is
    /// chosen because it comfortably exceeds the reported "5" cap.
    #[test]
    fn converts_all_plain_paragraph_references() {
        let n = 10;
        let xml = build_doc_xml_with_n_plain_references(n);
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-latex-test-plain-refs-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "plain-refs.docx", &xml);
        let out = dir.join("plain-refs.tex");
        let res = convert_docx_to_latex(&src, &out).expect("conversion should succeed");
        let latex = std::fs::read_to_string(&out).unwrap();

        // Every reference must appear in the output.
        for i in 1..=n {
            let needle = format!("Title number {}", i);
            assert!(
                latex.contains(&needle),
                "reference {} missing from LaTeX output (bug: converter is dropping references)",
                i
            );
        }
        // Explicitly verify the last reference is present — this is the
        // assertion that would have failed if there were a `.take(5)` cap.
        assert!(
            latex.contains("Title number 10"),
            "10th reference missing — converter is capping at fewer than 10"
        );
        // The body-paragraph counter should reflect all 10 references.
        assert_eq!(
            res.paragraphs, n,
            "paragraph count should match the number of plain-paragraph references"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Same property as the plain-paragraph test, but with numbered-list
    /// references (`<w:numPr>` with realistic `<w:ilvl>` + `<w:numId>`
    /// children, the exact shape MS Word emits). All n references should
    /// be collected into a single `itemize` environment.
    #[test]
    fn converts_all_numbered_list_references() {
        let n = 12;
        let xml = build_doc_xml_with_n_numbered_references(n);
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-latex-test-num-refs-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "num-refs.docx", &xml);
        let out = dir.join("num-refs.tex");
        let res = convert_docx_to_latex(&src, &out).expect("conversion should succeed");
        let latex = std::fs::read_to_string(&out).unwrap();

        for i in 1..=n {
            assert!(
                latex.contains(&format!("Title number {}", i)),
                "numbered-list reference {} missing from output",
                i
            );
        }
        assert!(
            latex.contains("Title number 12"),
            "12th numbered-list reference missing — converter is capping at fewer than 12"
        );
        // All numbered-list references should be collected into a single
        // itemize environment (the converter's "consecutive list paragraphs"
        // behavior). One \begin{itemize} for the whole run.
        let begin_count = latex.matches("\\begin{itemize}").count();
        assert_eq!(
            begin_count, 1,
            "expected one itemize environment for {} consecutive list items, found {}",
            n, begin_count
        );
        assert_eq!(res.list_items, n);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The actual root cause of "only 5 references converted": references
    /// stored in `word/footnotes.xml` (Chicago notes-bibliography,
    /// Vancouver with footnote citations, any "Insert Footnote" reference
    /// list) were silently dropped by the v2.2.1 r3 converter, which only
    /// read `word/document.xml`. The v2.2.2 r1 fix extracts footnotes and
    /// appends them under a `\section*{Footnotes}` heading. This test
    /// builds a .docx with 5 references in the body and 15 in footnotes
    /// and asserts all 20 appear in the output.
    #[test]
    fn extracts_footnote_references_not_just_document_xml() {
        let body_refs = 5;
        let footnote_refs = 15;
        let document_xml = build_doc_xml_with_n_plain_references(body_refs);
        let footnotes_xml = build_footnotes_xml_with_n_references(footnote_refs);
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-latex-test-fn-refs-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx_with_parts(
            &dir,
            "fn-refs.docx",
            &document_xml,
            Some(&footnotes_xml),
            None,
        );
        let out = dir.join("fn-refs.tex");
        let res = convert_docx_to_latex(&src, &out).expect("conversion should succeed");
        let latex = std::fs::read_to_string(&out).unwrap();

        // Every body reference must be present.
        for i in 1..=body_refs {
            let needle = format!("Title number {}", i);
            assert!(latex.contains(&needle), "body reference {} missing", i);
        }
        // Every footnote reference must also be present — this is the
        // regression. Without the v2.2.2 r1 fix, only the 5 body references
        // would be in the output, matching the user's "only 5 references"
        // observation.
        for i in 1..=footnote_refs {
            let needle = format!("Footnote reference {}", i);
            assert!(
                latex.contains(&needle),
                "footnote reference {} missing — the v2.2.2 r1 footnotes extraction regressed",
                i
            );
        }
        // The Footnotes section heading should be injected before the
        // footnote content.
        assert!(
            latex.contains("\\section*{Footnotes}"),
            "Footnotes section heading was not injected"
        );
        // A warning should explain what was extracted.
        assert!(
            res.warnings
                .iter()
                .any(|w| w.contains("footnote paragraph(s) were extracted")),
            "expected a footnote-extraction warning, got: {:?}",
            res.warnings
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Same property for endnotes. Endnotes are rarer than footnotes but
    /// the same silent-drop bug applied to them.
    #[test]
    fn extracts_endnote_references() {
        let body_refs = 3;
        let endnote_refs = 8;
        let document_xml = build_doc_xml_with_n_plain_references(body_refs);
        let endnotes_xml = build_endnotes_xml_with_n_references(endnote_refs);
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-latex-test-en-refs-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx_with_parts(
            &dir,
            "en-refs.docx",
            &document_xml,
            None,
            Some(&endnotes_xml),
        );
        let out = dir.join("en-refs.tex");
        let res = convert_docx_to_latex(&src, &out).expect("conversion should succeed");
        let latex = std::fs::read_to_string(&out).unwrap();

        for i in 1..=body_refs {
            assert!(latex.contains(&format!("Title number {}", i)));
        }
        for i in 1..=endnote_refs {
            assert!(
                latex.contains(&format!("Endnote reference {}", i)),
                "endnote reference {} missing",
                i
            );
        }
        assert!(latex.contains("\\section*{Endnotes}"));
        assert!(
            res.warnings
                .iter()
                .any(|w| w.contains("endnote paragraph(s) were extracted")),
            "expected an endnote-extraction warning, got: {:?}",
            res.warnings
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A .docx with no footnotes or endnotes parts must continue to work
    /// exactly as before — no spurious warnings, no spurious section
    /// headings. This guards the no-op path against the v2.2.2 r1 changes.
    #[test]
    fn no_warnings_when_docx_has_no_footnotes_or_endnotes() {
        let xml = build_doc_xml_with_n_plain_references(4);
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-latex-test-no-fn-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "no-fn.docx", &xml);
        let out = dir.join("no-fn.tex");
        let res = convert_docx_to_latex(&src, &out).expect("conversion should succeed");
        let latex = std::fs::read_to_string(&out).unwrap();

        assert!(
            !latex.contains("\\section*{Footnotes}"),
            "Footnotes heading should not appear when there are no footnotes"
        );
        assert!(
            !latex.contains("\\section*{Endnotes}"),
            "Endnotes heading should not appear when there are no endnotes"
        );
        assert!(
            !res
                .warnings
                .iter()
                .any(|w| w.contains("footnote paragraph(s) were extracted")),
            "no footnote-extraction warning expected"
        );
        assert!(
            !res
                .warnings
                .iter()
                .any(|w| w.contains("endnote paragraph(s) were extracted")),
            "no endnote-extraction warning expected"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
