//! .docx → LaTeX conversion (v2.2.1 r3).
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
//! - `Heading4`-`Heading6`      → `\paragraph{...}`
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
pub fn convert_docx_to_latex(path: &Path, output_path: &Path) -> Result<DocxToLatexResult, String> {
    let document_xml = read_document_xml_string(path)?;

    let (blocks, parse_info) = parse_blocks(&document_xml)?;
    let rendered = render(&blocks, &parse_info);

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

/// Read `word/document.xml` out of a .docx as a string (same zip-reading
/// approach as the provenance module's reader).
fn read_document_xml_string(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("not a readable .docx ({e})"))?;
    let mut raw: Option<Vec<u8>> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        if entry.name() == "word/document.xml" {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("read document.xml: {e}"))?;
            raw = Some(buf);
            break;
        }
    }
    let raw = raw.ok_or_else(|| {
        "word/document.xml not found in .docx; the file may be corrupt or not a real .docx"
            .to_string()
    })?;
    Ok(String::from_utf8_lossy(&raw).into_owned())
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

fn render(blocks: &[Block], info: &ParseInfo) -> Rendered {
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

    let mut unknown_styles: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut title: Option<String> = None;
    let mut counts = Counts::default();

    let mut i = 0usize;
    while i < blocks.len() {
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
}
