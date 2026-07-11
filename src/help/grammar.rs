use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::OnceLock,
};

use eframe::egui;

use crate::SPACE_MONO_NAME;

/// The full Pikchr grammar reference, assembled from the upstream per-topic
/// markdown pages into a single self-contained document. Bundled into the
/// binary so the in-app help works offline.
const PIKCHR_GRAMMAR_MD: &str = include_str!("../../assets/docs/pikchr_grammar_full.md");

/// Named egui font families registered at startup in `lib.rs`. Regular uses
/// SpaceMono; bold uses SpaceMono-Bold so `**bold**` renders with true weight.
const REG_FAMILY: &str = "SpaceMono";
const BOLD_FAMILY: &str = "SpaceMonoBold";
const GRAMMAR_PREVIEW_MAX_WIDTH_FRACTION: f32 = 0.80;
const GRAMMAR_PREVIEW_SCALE: f32 = 1.5;
const GRAMMAR_CODE_BLOCK_SPACING: f32 = 8.0;
const GRAMMAR_BLOCK_SPACING: f32 = 2.0;
const GRAMMAR_LAYOUT_OVERSCAN: f32 = 700.0;
const GRAMMAR_WIDTH_EPSILON: f32 = 0.5;

//
// The grammar document is parsed with pulldown-cmark *once* into a cached
// `Vec<Block>` (see `grammar_blocks`). Rendering then iterates the cached
// blocks, emitting one widget per block via `egui::text::LayoutJob` (so inline
// `**bold**`/`*italic*`/`` `code` `` are formatted correctly). This keeps the
// per-frame cost to a few hundred widgets with no markdown parsing or string
// re-allocation, which is what makes the window scroll smoothly.

#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    link_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Block {
    Heading {
        level: u8,
        /// 0-based index among all headings; used as the TOC / scroll target.
        idx: usize,
        spans: Vec<Span>,
    },
    Para(Vec<Span>),
    ListItem(Vec<Span>),
    /// A fenced code block; text preserves embedded newlines.
    Code(CodeBlock),
    /// Raw HTML converted to readable plain text. The bundled grammar doc has
    /// a few handwritten HTML tables; dropping them makes the help look broken.
    Html(String),
    TableRow(Vec<Vec<Span>>),
    Hr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodeBlock {
    idx: usize,
    text: String,
    info: CodeInfo,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CodeInfo {
    language: Option<String>,
    pikchr: bool,
    toggle: bool,
    source: bool,
    center: bool,
    indent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GrammarDoc {
    blocks: Vec<Block>,
    anchors: std::collections::HashMap<String, usize>,
}

#[derive(Clone, Default)]
pub(super) struct GrammarViewState {
    source_blocks: HashSet<usize>,
    initialized_blocks: HashSet<usize>,
    previews: HashMap<usize, GrammarPreviewCache>,
    layout: GrammarLayoutCache,
}

impl fmt::Debug for GrammarViewState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrammarViewState")
            .field("source_blocks", &self.source_blocks)
            .field("initialized_blocks", &self.initialized_blocks)
            .field("previews", &self.previews)
            .field("layout", &self.layout)
            .finish()
    }
}

#[derive(Clone, Debug, Default)]
struct GrammarLayoutCache {
    block_heights: Vec<f32>,
    heading_offsets: HashMap<usize, f32>,
    total_height: f32,
    wrap_width: Option<f32>,
}

impl GrammarLayoutCache {
    fn ensure(&mut self, blocks: &[Block], wrap_width: f32) {
        let width_changed = self
            .wrap_width
            .is_none_or(|cached| (cached - wrap_width).abs() > GRAMMAR_WIDTH_EPSILON);
        if width_changed || self.block_heights.len() != blocks.len() {
            self.block_heights = estimated_block_heights(blocks, wrap_width);
            self.wrap_width = Some(wrap_width);
        }
        self.rebuild_offsets(blocks);
    }

    fn update_height(&mut self, block_index: usize, height: f32) {
        if let Some(cached) = self.block_heights.get_mut(block_index) {
            *cached = height.max(1.0);
        }
    }

    fn rebuild_offsets(&mut self, blocks: &[Block]) {
        self.heading_offsets.clear();
        let mut y = 0.0;
        let mut i = 0;
        while i < blocks.len() {
            if let Some(idx) = heading_idx(&blocks[i]) {
                self.heading_offsets.insert(idx, y);
            }
            let end = render_group_end(blocks, i);
            y += self.block_heights.get(i).copied().unwrap_or(1.0) + GRAMMAR_BLOCK_SPACING;
            i = end;
        }
        self.total_height = y;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VisibleGroup {
    start: usize,
    end: usize,
    y: f32,
    height: f32,
}

#[derive(Clone)]
enum GrammarPreviewCache {
    Ready {
        texture: egui::TextureHandle,
        scale: f32,
        background: egui::Color32,
    },
    Error(String),
}

impl fmt::Debug for GrammarPreviewCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready {
                scale, background, ..
            } => f
                .debug_struct("Ready")
                .field("texture", &"TextureHandle(...)")
                .field("scale", scale)
                .field("background", background)
                .finish(),
            Self::Error(err) => f.debug_tuple("Error").field(err).finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TocEntry {
    level: u8,
    idx: usize,
    text: String,
}

/// Span-collecting context on the parser stack. Each variant owns the spans
/// accumulated for its block.
enum Ctx {
    Para(Vec<Span>),
    Heading(u8, usize, Vec<Span>),
    ListItem(Vec<Span>),
    Cell(Vec<Span>),
}

impl Ctx {
    fn spans_mut(&mut self) -> &mut Vec<Span> {
        match self {
            Ctx::Para(s) | Ctx::Heading(_, _, s) | Ctx::ListItem(s) | Ctx::Cell(s) => s,
        }
    }
}

fn heading_level(l: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel::*;
    match l {
        H1 => 1,
        H2 => 2,
        H3 => 3,
        H4 => 4,
        H5 => 5,
        H6 => 6,
    }
}

/// Parse the document into renderable blocks. Headings carry a stable `idx`
/// (0-based, in document order) that the TOC, anchors, and scroll-to-heading
/// share.
fn parse_doc(src: &str) -> GrammarDoc {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let normalized = normalize_doc_tables(src);
    let mut blocks: Vec<Block> = Vec::new();
    let mut anchors = std::collections::HashMap::new();
    let mut pending_anchors: Vec<String> = Vec::new();
    let mut ctx_stack: Vec<Ctx> = Vec::new();
    let mut bold = 0u32;
    let mut italic = 0u32;
    let mut link_target: Option<String> = None;
    let mut heading_counter = 0usize;
    let mut code_counter = 0usize;
    // `Some` while inside a fenced code block; Text/SoftBreak accumulate here.
    let mut code_buf: Option<CodeBlock> = None;
    let mut row_cells: Vec<Vec<Span>> = Vec::new();

    for event in Parser::new_ext(&normalized, opts) {
        match event {
            Event::Start(Tag::Paragraph) => ctx_stack.push(Ctx::Para(Vec::new())),
            Event::End(TagEnd::Paragraph) => {
                if let Some(Ctx::Para(s)) = ctx_stack.pop()
                    && !s.is_empty()
                {
                    blocks.push(Block::Para(s));
                }
            },

            Event::Start(Tag::Heading { level, .. }) => {
                let idx = heading_counter;
                heading_counter += 1;
                for anchor in pending_anchors.drain(..) {
                    anchors.insert(anchor, idx);
                }
                ctx_stack.push(Ctx::Heading(heading_level(level), idx, Vec::new()));
            },
            Event::End(TagEnd::Heading(_)) => {
                if let Some(Ctx::Heading(level, idx, spans)) = ctx_stack.pop() {
                    if is_table_row_text(&plain_text(&spans)) {
                        blocks.push(Block::Para(spans));
                    } else {
                        blocks.push(Block::Heading { level, idx, spans });
                    }
                }
            },

            Event::Start(Tag::List(_)) => {},
            Event::End(TagEnd::List(_)) => {},
            Event::Start(Tag::Item) => ctx_stack.push(Ctx::ListItem(Vec::new())),
            Event::End(TagEnd::Item) => {
                if let Some(Ctx::ListItem(s)) = ctx_stack.pop() {
                    blocks.push(Block::ListItem(s));
                }
            },

            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(info) => parse_code_info(info.as_ref()),
                    pulldown_cmark::CodeBlockKind::Indented => CodeInfo::default(),
                };
                code_buf = Some(CodeBlock {
                    idx: code_counter,
                    text: String::new(),
                    info,
                });
                code_counter += 1;
            },
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = code_buf.take() {
                    blocks.push(Block::Code(block));
                }
            },

            Event::Start(Tag::Table(_)) | Event::End(TagEnd::Table) => {},
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                row_cells.clear();
            },
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if !row_cells.is_empty() {
                    blocks.push(Block::TableRow(std::mem::take(&mut row_cells)));
                }
            },
            Event::Start(Tag::TableCell) => ctx_stack.push(Ctx::Cell(Vec::new())),
            Event::End(TagEnd::TableCell) => {
                if let Some(Ctx::Cell(s)) = ctx_stack.pop() {
                    row_cells.push(s);
                }
            },

            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => link_target = Some(dest_url.to_string()),
            Event::End(TagEnd::Link) => link_target = None,

            Event::Text(t) => {
                if let Some(buf) = code_buf.as_mut() {
                    buf.text.push_str(t.as_ref());
                } else {
                    push_span(
                        &mut ctx_stack,
                        &t,
                        bold > 0,
                        italic > 0,
                        false,
                        link_target.clone(),
                    );
                }
            },
            Event::Code(t) => {
                push_span(
                    &mut ctx_stack,
                    &t,
                    bold > 0,
                    italic > 0,
                    true,
                    link_target.clone(),
                );
            },
            Event::Html(t) => {
                pending_anchors.extend(extract_anchor_ids(&t));
                let text = html_to_text(&t);
                if !text.trim().is_empty() {
                    blocks.push(Block::Html(text));
                }
            },
            Event::InlineHtml(t) => {
                pending_anchors.extend(extract_anchor_ids(&t));
                let text = html_to_text(&t);
                if !text.is_empty() {
                    push_span(
                        &mut ctx_stack,
                        &text,
                        bold > 0,
                        italic > 0,
                        false,
                        link_target.clone(),
                    );
                }
            },
            Event::SoftBreak | Event::HardBreak => {
                if let Some(buf) = code_buf.as_mut() {
                    buf.text.push('\n');
                } else if let Some(ctx) = ctx_stack.last_mut() {
                    let spans = ctx.spans_mut();
                    if let Some(last) = spans.last_mut() {
                        last.text.push('\n');
                    } else {
                        spans.push(Span {
                            text: "\n".into(),
                            bold: false,
                            italic: false,
                            code: false,
                            link_target: None,
                        });
                    }
                }
            },
            Event::Rule => blocks.push(Block::Hr),

            // Ignore everything else (footnotes, task-list markers, etc.).
            _ => {},
        }
    }

    GrammarDoc { blocks, anchors }
}

#[cfg(test)]
fn parse_blocks(src: &str) -> Vec<Block> {
    parse_doc(src).blocks
}

fn parse_code_info(info: &str) -> CodeInfo {
    let mut parsed = CodeInfo::default();
    let mut tokens = info.split_whitespace();
    parsed.language = tokens.next().map(str::to_owned);

    for token in info.split_whitespace() {
        match token {
            "pikchr" => parsed.pikchr = true,
            "toggle" => parsed.toggle = true,
            "source" => parsed.source = true,
            "center" => parsed.center = true,
            "indent" => parsed.indent = true,
            _ => {},
        }
    }

    parsed
}

fn normalize_doc_tables(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut lines = src.lines().peekable();
    let mut in_fence = false;
    let mut fence = "";

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let mark = &trimmed[..3];
            if in_fence && mark == fence {
                in_fence = false;
                fence = "";
            } else if !in_fence {
                in_fence = true;
                fence = mark;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if !in_fence && is_table_row_text(line) {
            out.push_str(&normalize_table_row(line));
        } else {
            out.push_str(line);
        }
        out.push('\n');

        if !in_fence
            && is_table_row_text(line)
            && lines
                .peek()
                .is_some_and(|next| is_legacy_table_separator(next))
        {
            let _ = lines.next();
            out.push_str(&gfm_table_separator(line));
            out.push('\n');
        }
    }

    out
}

fn normalize_table_row(row: &str) -> String {
    decode_entities(row)
        .replace('\u{00A0}', " ")
        .replace("|:", "|")
        .replace(":|", "|")
}

fn is_legacy_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '-')
}

fn gfm_table_separator(header: &str) -> String {
    let columns = pipe_column_count(header).max(1);
    let mut out = String::new();
    out.push('|');
    for _ in 0..columns {
        out.push_str(" --- |");
    }
    out
}

fn pipe_column_count(row: &str) -> usize {
    let trimmed = row.trim();
    let count = trimmed.matches('|').count();
    if trimmed.starts_with('|') && trimmed.ends_with('|') {
        count.saturating_sub(1)
    } else {
        count + 1
    }
}

/// Decode a span into the nearest enclosing context's span list with the
/// current inline flags, after expanding HTML entities.
fn push_span(
    ctx_stack: &mut [Ctx],
    text: &str,
    bold: bool,
    italic: bool,
    code: bool,
    link_target: Option<String>,
) {
    let Some(ctx) = ctx_stack.last_mut() else {
        return;
    };
    ctx.spans_mut().push(Span {
        text: decode_entities(text),
        bold,
        italic,
        code,
        link_target,
    });
}

/// Expand the HTML entities that appear in the Pikchr docs (`&rarr;`,
/// `&nbsp;`, `&#9654;`, …) to their Unicode characters. Anything that is not a
/// recognized entity is left verbatim.
fn decode_entities(input: &str) -> String {
    const NAMED: &[(&str, &str)] = &[
        ("amp", "&"),
        ("lt", "<"),
        ("gt", ">"),
        ("quot", "\""),
        ("apos", "'"),
        ("nbsp", "\u{00A0}"),
        ("rarr", "\u{2192}"),
        ("larr", "\u{2190}"),
        ("uarr", "\u{2191}"),
        ("darr", "\u{2193}"),
        ("harr", "\u{2194}"),
        ("mdash", "\u{2014}"),
        ("ndash", "\u{2013}"),
        ("hellip", "\u{2026}"),
        ("copy", "\u{00A9}"),
        ("reg", "\u{00AE}"),
        ("trade", "\u{2122}"),
        ("deg", "\u{00B0}"),
        ("times", "\u{00D7}"),
        ("divide", "\u{00F7}"),
        ("plusmn", "\u{00B1}"),
        ("le", "\u{2264}"),
        ("ge", "\u{2265}"),
        ("ne", "\u{2260}"),
        ("asymp", "\u{2248}"),
        ("infin", "\u{221E}"),
        ("alpha", "\u{03B1}"),
        ("beta", "\u{03B2}"),
        ("gamma", "\u{03B3}"),
        ("delta", "\u{03B4}"),
        ("pi", "\u{03C0}"),
        ("sigma", "\u{03C3}"),
        ("tau", "\u{03C4}"),
        ("omega", "\u{03C9}"),
        ("sum", "\u{2211}"),
        ("prod", "\u{220F}"),
    ];
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';')
            && semi <= 12
        {
            let body = &after[..semi];
            let matched = if let Some(num) = body.strip_prefix('#') {
                let code = if let Some(hex) = num.strip_prefix(['x', 'X']) {
                    u32::from_str_radix(hex, 16)
                } else {
                    num.parse::<u32>()
                };
                match code.ok().and_then(char::from_u32) {
                    Some(c) => {
                        out.push(c);
                        true
                    },
                    None => false,
                }
            } else if let Some((_, v)) = NAMED.iter().find(|(n, _)| *n == body) {
                out.push_str(v);
                true
            } else {
                false
            };
            if matched {
                rest = &after[semi + 1..];
                continue;
            }
        }
        // Not a recognized entity: emit the '&' literally and keep scanning.
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn html_to_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '<' {
            out.push(ch);
            continue;
        }

        let mut tag = String::new();
        for tag_ch in chars.by_ref() {
            if tag_ch == '>' {
                break;
            }
            tag.push(tag_ch);
        }
        let tag_name = tag
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("");
        match tag_name {
            "a" => {},
            "blockquote" | "table" => {
                if !out.ends_with('\n') && !out.trim().is_empty() {
                    out.push('\n');
                }
            },
            "tr" => {
                if !out.ends_with('\n') && !out.trim().is_empty() {
                    out.push('\n');
                }
            },
            "td" | "th" => {
                let trimmed = out.trim_end();
                if !trimmed.is_empty() && !trimmed.ends_with('|') && !trimmed.ends_with('\n') {
                    out.push_str(" | ");
                }
            },
            _ => {},
        }
    }
    decode_entities(&out)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_anchor_ids(input: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = input;
    while let Some(pos) = rest.find("id=\"") {
        let after = &rest[pos + 4..];
        let Some(end) = after.find('"') else {
            break;
        };
        ids.push(after[..end].to_owned());
        rest = &after[end + 1..];
    }
    ids
}

fn grammar_doc() -> &'static GrammarDoc {
    static DOC: OnceLock<GrammarDoc> = OnceLock::new();
    DOC.get_or_init(|| parse_doc(PIKCHR_GRAMMAR_MD))
}

fn grammar_blocks() -> &'static [Block] {
    &grammar_doc().blocks
}

fn grammar_link_target(target: &str) -> Option<usize> {
    let anchor = target.strip_prefix('#')?;
    grammar_doc().anchors.get(anchor).copied()
}

fn grammar_toc() -> &'static [TocEntry] {
    static TOC: OnceLock<Vec<TocEntry>> = OnceLock::new();
    TOC.get_or_init(|| {
        grammar_blocks()
            .iter()
            .filter_map(|b| match b {
                Block::Heading { level, idx, spans } => Some(TocEntry {
                    level: *level,
                    idx: *idx,
                    text: toc_text(spans),
                }),
                _ => None,
            })
            .collect()
    })
}

fn toc_text(spans: &[Span]) -> String {
    plain_text(spans)
        .replace("\u{25B6}info", "")
        .trim()
        .to_owned()
}

fn plain_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

fn heading_idx(block: &Block) -> Option<usize> {
    match block {
        Block::Heading { idx, .. } => Some(*idx),
        _ => None,
    }
}

fn render_group_end(blocks: &[Block], start: usize) -> usize {
    if matches!(blocks.get(start), Some(Block::TableRow(_))) {
        let mut end = start + 1;
        while matches!(blocks.get(end), Some(Block::TableRow(_))) {
            end += 1;
        }
        end
    } else {
        start + 1
    }
}

fn estimated_block_heights(blocks: &[Block], wrap_width: f32) -> Vec<f32> {
    let mut heights = vec![1.0; blocks.len()];
    let mut i = 0;
    while i < blocks.len() {
        let end = render_group_end(blocks, i);
        heights[i] = estimated_group_height(&blocks[i..end], wrap_width);
        i = end;
    }
    heights
}

fn visible_groups(
    blocks: &[Block],
    heights: &[f32],
    visible_min: f32,
    visible_max: f32,
) -> Vec<VisibleGroup> {
    let mut out = Vec::new();
    let mut y = 0.0;
    let mut i = 0;
    while i < blocks.len() {
        let end = render_group_end(blocks, i);
        let height = heights.get(i).copied().unwrap_or(1.0);
        let block_min = y;
        let block_max = y + height;
        if block_max >= visible_min && block_min <= visible_max {
            out.push(VisibleGroup {
                start: i,
                end,
                y,
                height,
            });
        }
        y += height + GRAMMAR_BLOCK_SPACING;
        i = end;
    }
    out
}

#[cfg(feature = "perf-workloads")]
pub(crate) fn perf_layout_workload(wrap_width: f32, viewports: usize) -> usize {
    let blocks = grammar_blocks();
    let heights = estimated_block_heights(blocks, wrap_width);
    let total = heights.iter().sum::<f32>() + blocks.len() as f32 * GRAMMAR_BLOCK_SPACING;
    (0..viewports)
        .map(|step| {
            let min = total * step as f32 / viewports.max(1) as f32;
            visible_groups(blocks, &heights, min, min + 900.0).len()
        })
        .sum()
}

fn estimated_group_height(blocks: &[Block], wrap_width: f32) -> f32 {
    let Some(block) = blocks.first() else {
        return 1.0;
    };
    match block {
        Block::Heading { level, spans, .. } => {
            let size = match *level {
                1 => 28.0,
                2 => 24.0,
                3 => 21.0,
                _ => 18.0,
            };
            estimate_wrapped_text_height(spans, wrap_width, size)
        },
        Block::Para(spans) => estimate_wrapped_text_height(spans, wrap_width, 18.0),
        Block::ListItem(spans) => estimate_wrapped_text_height(spans, wrap_width - 24.0, 18.0),
        Block::Code(block) => estimated_code_height(block, wrap_width),
        Block::Html(text) => estimate_plain_text_height(text, wrap_width, 17.0),
        Block::TableRow(_) => {
            let row_count = blocks
                .iter()
                .filter(|block| matches!(block, Block::TableRow(_)))
                .count();
            (row_count as f32 * 24.0).max(24.0)
        },
        Block::Hr => 8.0,
    }
}

fn estimate_wrapped_text_height(spans: &[Span], wrap_width: f32, line_height: f32) -> f32 {
    estimate_plain_text_height(&plain_text(spans), wrap_width, line_height)
}

fn estimate_plain_text_height(text: &str, wrap_width: f32, line_height: f32) -> f32 {
    let chars_per_line = (wrap_width.max(80.0) / 7.0).max(12.0);
    let lines = text
        .lines()
        .map(|line| {
            ((line.chars().count() as f32) / chars_per_line)
                .ceil()
                .max(1.0)
        })
        .sum::<f32>()
        .max(1.0);
    lines * line_height
}

fn estimated_code_height(block: &CodeBlock, wrap_width: f32) -> f32 {
    if block.info.pikchr && !block.info.source {
        (wrap_width * 0.25).clamp(90.0, 220.0) + GRAMMAR_CODE_BLOCK_SPACING * 2.0
    } else {
        let lines = block.text.lines().count().max(1) as f32;
        lines * 15.0 + GRAMMAR_CODE_BLOCK_SPACING * 2.0
    }
}

fn is_table_row_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('|') && trimmed.matches('|').count() >= 2
}

/// Build a wrapped `LayoutJob` from parsed spans: bold uses the SpaceMono-Bold
/// family (true weight), italic tilts glyphs, inline code gets a faint
/// background, links take the accent color + underline.
fn build_job(
    spans: &[Span],
    size: f32,
    base_color: egui::Color32,
    accent: egui::Color32,
    code_bg: egui::Color32,
    wrap_width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    for s in spans {
        let family = if s.bold { BOLD_FAMILY } else { REG_FAMILY };
        let color = if s.link_target.is_some() {
            accent
        } else {
            base_color
        };
        let format = egui::text::TextFormat {
            font_id: egui::FontId::new(size, egui::FontFamily::Name(family.into())),
            color,
            italics: s.italic,
            background: if s.code {
                code_bg
            } else {
                egui::Color32::TRANSPARENT
            },
            underline: if s.link_target.is_some() {
                egui::Stroke::new(1.0, accent)
            } else {
                egui::Stroke::NONE
            },
            ..Default::default()
        };
        job.append(&s.text, 0.0, format);
    }
    job
}

fn has_links(spans: &[Span]) -> bool {
    spans.iter().any(|s| s.link_target.is_some())
}

fn rich_span(
    span: &Span,
    size: f32,
    base_color: egui::Color32,
    accent: egui::Color32,
) -> egui::RichText {
    let family = if span.bold { BOLD_FAMILY } else { REG_FAMILY };
    let color = if span.link_target.is_some() {
        accent
    } else {
        base_color
    };
    let mut text = egui::RichText::new(&span.text)
        .font(egui::FontId::new(
            size,
            egui::FontFamily::Name(family.into()),
        ))
        .color(color);
    if span.italic {
        text = text.italics();
    }
    text
}

fn render_linked_spans(
    ui: &mut egui::Ui,
    spans: &[Span],
    size: f32,
    base_color: egui::Color32,
    accent: egui::Color32,
    scroll_target: &mut Option<usize>,
) -> egui::Response {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for span in spans {
            let rich = rich_span(span, size, base_color, accent);
            if let Some(target) = span.link_target.as_deref() {
                let response = ui.add(egui::Button::new(rich).frame(false));
                if response.clicked()
                    && let Some(idx) = grammar_link_target(target)
                {
                    *scroll_target = Some(idx);
                }
            } else {
                ui.label(rich);
            }
        }
    })
    .response
}

/// Render the Grammar view: a resizable left sidebar TOC and the markdown body.
/// `scroll_target` is read/written by both panels so a TOC click scrolls the
/// body in the same frame.
pub(super) fn render_grammar(
    ui: &mut egui::Ui,
    scroll_target: &mut Option<usize>,
    view: &mut GrammarViewState,
) {
    fn reg_family() -> egui::FontFamily {
        egui::FontFamily::Name(REG_FAMILY.into())
    }
    let style = GrammarRenderStyle {
        accent: ui.visuals().hyperlink_color,
        body_color: ui.visuals().text_color(),
        code_bg: ui.visuals().faint_bg_color,
        dim: ui.visuals().weak_text_color(),
        family: reg_family(),
    };

    egui::SidePanel::left("grammar_toc")
        .resizable(true)
        .width_range(140.0..=340.0)
        .default_width(210.0)
        .show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Contents")
                    .font(egui::FontId::new(14.0, reg_family()))
                    .color(style.accent),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for entry in grammar_toc() {
                        // The bundled document appends linked articles after
                        // the grammar. Show the grammar productions and article
                        // titles, but keep article-local subsections in the body.
                        if entry.level > 3 {
                            continue;
                        }
                        let indent = (entry.level as f32 - 1.0) * 8.0;
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            let rich = egui::RichText::new(&entry.text)
                                .font(egui::FontId::new(11.0, reg_family()));
                            if ui.add(egui::Button::new(rich).frame(false)).clicked() {
                                *scroll_target = Some(entry.idx);
                            }
                        });
                    }
                });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        render_grammar_body(ui, scroll_target, view, &style);
    });
}

struct GrammarRenderStyle {
    accent: egui::Color32,
    body_color: egui::Color32,
    code_bg: egui::Color32,
    dim: egui::Color32,
    family: egui::FontFamily,
}

fn render_grammar_body(
    ui: &mut egui::Ui,
    scroll_target: &mut Option<usize>,
    view: &mut GrammarViewState,
    style: &GrammarRenderStyle,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            let wrap = ui.available_width();
            let blocks = grammar_blocks();
            view.layout.ensure(blocks, wrap);
            ui.set_height(view.layout.total_height);

            if let Some(target) = scroll_target.take() {
                if let Some(y) = view.layout.heading_offsets.get(&target).copied() {
                    let rect = egui::Rect::from_min_size(
                        ui.max_rect().min + egui::vec2(0.0, y),
                        egui::vec2(wrap, 1.0),
                    );
                    ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                } else {
                    *scroll_target = Some(target);
                }
            }

            let groups = visible_groups(
                blocks,
                &view.layout.block_heights,
                (viewport.min.y - GRAMMAR_LAYOUT_OVERSCAN).max(0.0),
                viewport.max.y + GRAMMAR_LAYOUT_OVERSCAN,
            );
            let content_top = ui.max_rect().top();

            for group in groups {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(ui.max_rect().left(), content_top + group.y),
                    egui::vec2(wrap, group.height),
                );
                let measured = ui
                    .scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                        |ui| {
                            render_grammar_group(
                                ui,
                                &blocks[group.start..group.end],
                                scroll_target,
                                view,
                                style,
                            );
                            ui.add_space(GRAMMAR_BLOCK_SPACING);
                            ui.min_rect().height()
                        },
                    )
                    .inner;
                view.layout.update_height(group.start, measured);
            }
            view.layout.rebuild_offsets(blocks);
        });
}

fn render_grammar_group(
    ui: &mut egui::Ui,
    blocks: &[Block],
    scroll_target: &mut Option<usize>,
    view: &mut GrammarViewState,
    style: &GrammarRenderStyle,
) {
    let Some(block) = blocks.first() else {
        return;
    };
    match block {
        Block::Heading { level, spans, .. } => {
            let size = match *level {
                1 => 18.0,
                2 => 16.0,
                3 => 14.0,
                _ => 12.5,
            };
            if has_links(spans) {
                render_linked_spans(ui, spans, size, style.accent, style.accent, scroll_target);
            } else {
                let job = build_job(
                    spans,
                    size,
                    style.accent,
                    style.accent,
                    style.code_bg,
                    ui.available_width(),
                );
                ui.add(egui::Label::new(job).selectable(false));
            }
        },
        Block::Para(spans) => {
            if has_links(spans) {
                render_linked_spans(
                    ui,
                    spans,
                    12.0,
                    style.body_color,
                    style.accent,
                    scroll_target,
                );
            } else {
                let job = build_job(
                    spans,
                    12.0,
                    style.body_color,
                    style.accent,
                    style.code_bg,
                    ui.available_width(),
                );
                ui.add(egui::Label::new(job).selectable(false));
            }
        },
        Block::ListItem(spans) => {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("\u{2022}")
                        .font(egui::FontId::new(12.0, style.family.clone()))
                        .color(style.body_color),
                );
                if has_links(spans) {
                    render_linked_spans(
                        ui,
                        spans,
                        12.0,
                        style.body_color,
                        style.accent,
                        scroll_target,
                    );
                } else {
                    let job = build_job(
                        spans,
                        12.0,
                        style.body_color,
                        style.accent,
                        style.code_bg,
                        ui.available_width(),
                    );
                    ui.add(egui::Label::new(job).selectable(false));
                }
            });
        },
        Block::Code(block) => render_code_block(ui, block, view, style.dim, style.family.clone()),
        Block::Html(text) => {
            ui.label(
                egui::RichText::new(text.as_str())
                    .font(egui::FontId::new(11.5, style.family.clone()))
                    .color(style.body_color),
            );
        },
        Block::TableRow(_) => render_table(
            ui,
            blocks,
            style.body_color,
            style.accent,
            style.code_bg,
            style.family.clone(),
        ),
        Block::Hr => {
            ui.separator();
        },
    }
}

fn render_code_block(
    ui: &mut egui::Ui,
    block: &CodeBlock,
    view: &mut GrammarViewState,
    dim: egui::Color32,
    family: egui::FontFamily,
) {
    ui.add_space(GRAMMAR_CODE_BLOCK_SPACING);
    if !block.info.pikchr {
        render_code_source(ui, block.text.as_str(), dim, family, false);
        ui.add_space(GRAMMAR_CODE_BLOCK_SPACING);
        return;
    }

    if block.info.toggle {
        let showing_source = code_block_showing_source(block, view);
        if showing_source {
            if render_code_source(ui, block.text.as_str(), dim, family, true).clicked() {
                view.source_blocks.remove(&block.idx);
            }
        } else {
            render_pikchr_preview(ui, block, view);
        }
    } else {
        render_pikchr_preview(ui, block, view);
    }
    ui.add_space(GRAMMAR_CODE_BLOCK_SPACING);
}

fn code_block_showing_source(block: &CodeBlock, view: &mut GrammarViewState) -> bool {
    if view.initialized_blocks.insert(block.idx) && block.info.source {
        view.source_blocks.insert(block.idx);
    }
    view.source_blocks.contains(&block.idx)
}

fn render_code_source(
    ui: &mut egui::Ui,
    text: &str,
    dim: egui::Color32,
    family: egui::FontFamily,
    clickable: bool,
) -> egui::Response {
    let label = egui::Label::new(
        egui::RichText::new(text)
            .font(egui::FontId::new(11.0, family))
            .color(dim),
    )
    .selectable(true);
    let response = ui.add(label);
    if clickable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

fn render_pikchr_preview(ui: &mut egui::Ui, block: &CodeBlock, view: &mut GrammarViewState) {
    let background = ui.visuals().window_fill();
    let raster_scale = GRAMMAR_PREVIEW_SCALE;
    let needs_preview = match view.previews.get(&block.idx) {
        Some(GrammarPreviewCache::Ready {
            background: cached,
            scale: cached_scale,
            ..
        }) => *cached != background || (*cached_scale - raster_scale).abs() > f32::EPSILON,
        Some(GrammarPreviewCache::Error(_)) => false,
        None => true,
    };
    if needs_preview {
        let preview = build_pikchr_preview(ui, block, background, raster_scale);
        view.previews.insert(block.idx, preview);
    }

    match view.previews.get(&block.idx) {
        Some(GrammarPreviewCache::Ready { texture, scale, .. }) => {
            let draw_size =
                grammar_preview_display_size(texture.size_vec2(), *scale, ui.clip_rect().width());
            let image = egui::Image::new(texture).fit_to_exact_size(draw_size).uv(
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            );

            let response = ui.add(image.sense(egui::Sense::click()));
            if block.info.toggle && response.clicked() {
                view.source_blocks.insert(block.idx);
            }
        },
        Some(GrammarPreviewCache::Error(err)) => {
            render_code_source(
                ui,
                block.text.as_str(),
                ui.visuals().weak_text_color(),
                egui::FontFamily::Name(REG_FAMILY.into()),
                false,
            );
            ui.label(
                egui::RichText::new(err)
                    .font(egui::FontId::new(
                        11.0,
                        egui::FontFamily::Name(REG_FAMILY.into()),
                    ))
                    .color(ui.visuals().error_fg_color),
            );
        },
        None => {},
    }
}

fn build_pikchr_preview(
    ui: &egui::Ui,
    block: &CodeBlock,
    background: egui::Color32,
    raster_scale: f32,
) -> GrammarPreviewCache {
    let svg = match render_pikchr_svg(block) {
        Ok(svg) => svg,
        Err(err) => return GrammarPreviewCache::Error(err),
    };
    let image = match render_pikchr_image_from_svg(&svg, background, raster_scale) {
        Ok(image) => image,
        Err(err) => return GrammarPreviewCache::Error(err),
    };
    let texture = ui.ctx().load_texture(
        format!("grammar_pikchr_{}", block.idx),
        image,
        egui::TextureOptions::LINEAR,
    );
    GrammarPreviewCache::Ready {
        texture,
        scale: raster_scale,
        background,
    }
}

fn grammar_preview_display_size(
    texture_size: egui::Vec2,
    raster_scale: f32,
    content_width: f32,
) -> egui::Vec2 {
    let logical_size = texture_size / raster_scale.max(1.0);
    let logical_size = egui::vec2(logical_size.x.max(1.0), logical_size.y.max(1.0));
    let max_width = (content_width.max(1.0) * GRAMMAR_PREVIEW_MAX_WIDTH_FRACTION).max(1.0);
    let fit = (max_width / logical_size.x).min(1.0);
    logical_size * fit
}

#[cfg(test)]
fn render_pikchr_image(
    block: &CodeBlock,
    background: egui::Color32,
) -> Result<egui::ColorImage, String> {
    let svg = render_pikchr_svg(block)?;
    render_pikchr_image_from_svg(&svg, background, 1.0)
}

fn render_pikchr_image_from_svg(
    svg: &str,
    background: egui::Color32,
    raster_scale: f32,
) -> Result<egui::ColorImage, String> {
    crate::image::render_svg_to_image(
        svg,
        raster_scale,
        crate::image::RenderBackground::Color(background),
    )
    .ok_or_else(|| "Could not rasterize Pikchr preview".to_owned())
}

fn render_pikchr_svg(block: &CodeBlock) -> Result<String, String> {
    let svg = pikchr_pro::pikchr::render_pikchr(pikchr_pro::types::PikchrCode::new(&block.text))
        .map_err(|err| err.inner_string())?;
    let svg = svg.inject_svg_style(SPACE_MONO_NAME).into_inner();
    Ok(crate::image::sanitize_svg_for_usvg(&svg).into_owned())
}

fn render_table(
    ui: &mut egui::Ui,
    rows: &[Block],
    body_color: egui::Color32,
    accent: egui::Color32,
    code_bg: egui::Color32,
    family: egui::FontFamily,
) {
    let max_cols = rows
        .iter()
        .filter_map(|row| match row {
            Block::TableRow(cells) => Some(cells.len()),
            _ => None,
        })
        .max()
        .unwrap_or(1)
        .max(1);
    let pane_width = ui.available_width().max(1.0);
    let spacing = 18.0;
    let (table_width, cell_width) = table_layout_widths(pane_width, max_cols, spacing);

    ui.horizontal(|ui| {
        ui.add_space(((pane_width - table_width) / 2.0).max(0.0));
        ui.allocate_ui_with_layout(
            egui::vec2(table_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::Grid::new(("grammar_table", rows.as_ptr(), table_width.to_bits()))
                    .striped(false)
                    .num_columns(max_cols)
                    .min_col_width(cell_width)
                    .max_col_width(cell_width)
                    .spacing([spacing, 4.0])
                    .show(ui, |ui| {
                        for (row_idx, row) in rows.iter().enumerate() {
                            let Block::TableRow(cells) = row else {
                                continue;
                            };
                            for col in 0..max_cols {
                                if let Some(cell) = cells.get(col) {
                                    let color = if row_idx == 0 { accent } else { body_color };
                                    let size = if row_idx == 0 { 11.5 } else { 11.0 };
                                    let job =
                                        build_job(cell, size, color, accent, code_bg, cell_width);
                                    let frame = if row_idx > 0 && row_idx % 2 == 1 {
                                        egui::Frame::new().fill(ui.visuals().faint_bg_color)
                                    } else {
                                        egui::Frame::new()
                                    };
                                    frame.show(ui, |ui| {
                                        ui.set_min_width(cell_width);
                                        ui.set_max_width(cell_width);
                                        ui.add(egui::Label::new(job).wrap().selectable(false));
                                    });
                                } else {
                                    let frame = if row_idx > 0 && row_idx % 2 == 1 {
                                        egui::Frame::new().fill(ui.visuals().faint_bg_color)
                                    } else {
                                        egui::Frame::new()
                                    };
                                    frame.show(ui, |ui| {
                                        ui.set_min_width(cell_width);
                                        ui.set_max_width(cell_width);
                                        ui.label(
                                            egui::RichText::new("")
                                                .font(egui::FontId::new(11.0, family.clone())),
                                        );
                                    });
                                }
                            }
                            ui.end_row();
                        }
                    });
            },
        );
    });
}

fn table_layout_widths(pane_width: f32, max_cols: usize, spacing: f32) -> (f32, f32) {
    let table_width = (pane_width.max(1.0) * 0.85).max(1.0);
    let max_cols = max_cols.max(1);
    let cell_width =
        ((table_width - spacing * (max_cols.saturating_sub(1) as f32)) / max_cols as f32).max(1.0);
    (table_width, cell_width)
}

#[cfg(test)]
mod tests {
    use super::{
        Block, CodeBlock, CodeInfo, GrammarViewState, PIKCHR_GRAMMAR_MD, Span,
        code_block_showing_source, decode_entities, estimated_block_heights, gfm_table_separator,
        grammar_blocks, grammar_link_target, grammar_preview_display_size, grammar_toc,
        is_table_row_text, normalize_table_row, parse_blocks, render_group_end,
        render_pikchr_image, render_pikchr_svg, table_layout_widths, toc_text, visible_groups,
    };

    #[test]
    fn bundled_grammar_doc_is_present_and_well_formed() {
        assert!(!PIKCHR_GRAMMAR_MD.is_empty(), "grammar doc is empty");
        assert!(
            PIKCHR_GRAMMAR_MD.starts_with("# Pikchr Grammar"),
            "grammar doc is missing its H1 title"
        );
        for needle in [
            "## *statement-list*",
            "## *statement*",
            "## *attribute*",
            "## *position*",
            "## *expr*",
        ] {
            assert!(
                PIKCHR_GRAMMAR_MD.contains(needle),
                "grammar doc is missing section header {needle:?}"
            );
        }
    }

    #[test]
    fn toc_excludes_fenced_comment_lines() {
        let toc = grammar_toc();
        assert!(toc.iter().any(|e| e.text == "Pikchr Grammar"), "missing H1");
        assert!(
            !toc.iter().any(|e| e.text == "Start and end blocks"),
            "fenced pikchr comment leaked into TOC"
        );
    }

    #[test]
    fn toc_keeps_grammar_productions_and_reference_titles_visible() {
        let visible: Vec<_> = grammar_toc()
            .iter()
            .filter(|e| e.level <= 3)
            .map(|e| e.text.as_str())
            .collect();
        assert!(
            visible.iter().any(|e| e.starts_with("statement-list")),
            "main grammar production is missing from visible TOC"
        );
        assert!(
            visible.iter().any(|e| e.starts_with("dot-property")),
            "late grammar production is missing from visible TOC"
        );
        assert!(
            visible.contains(&"Linked reference articles"),
            "linked-doc appendix is missing from visible TOC"
        );
        assert!(
            visible.contains(&"statement-list"),
            "linked article title is missing from visible TOC"
        );
        assert!(
            !visible.contains(&"Rules"),
            "article-local subsection leaked into visible TOC"
        );
        assert!(
            !visible.iter().any(|e| e.starts_with('|')),
            "table row leaked into visible TOC: {visible:?}"
        );
    }

    #[test]
    fn toc_text_drops_info_link_markers() {
        let spans = [Span {
            text: "*statement-list*: \u{25B6}info".into(),
            bold: false,
            italic: true,
            code: false,
            link_target: Some("#reference-stmtlist.md".into()),
        }];
        assert_eq!(toc_text(&spans), "*statement-list*:");
    }

    #[test]
    fn table_width_is_capped_to_content_fraction() {
        let pane_width = 1000.0;
        let (table_width, cell_width) = table_layout_widths(pane_width, 3, 18.0);
        assert_eq!(table_width, 850.0);
        assert!(cell_width <= table_width / 3.0);
    }

    #[test]
    fn visible_groups_select_only_intersecting_block_ranges() {
        let blocks = parse_blocks(
            "# One\n\nfirst\n\n| A | B |\n| --- | --- |\n| C | D |\n\nsecond\n\n# Two\n",
        );
        let mut heights = vec![20.0; blocks.len()];
        let table_start = blocks
            .iter()
            .position(|block| matches!(block, Block::TableRow(_)))
            .expect("table row");
        heights[table_start] = 60.0;

        let visible = visible_groups(&blocks, &heights, 24.0, 86.0);
        assert!(
            visible.iter().any(|group| group.start == 1),
            "paragraph intersecting viewport should be visible: {visible:?}"
        );
        assert!(
            visible.iter().any(|group| group.start == table_start),
            "table group intersecting viewport should be visible: {visible:?}"
        );
        assert!(
            visible
                .iter()
                .all(|group| group.start != 0 && group.start < blocks.len() - 1),
            "non-intersecting headings should be skipped: {visible:?}"
        );
        assert_eq!(visible[1].end, render_group_end(&blocks, table_start));
    }

    #[test]
    fn layout_cache_tracks_heading_offsets() {
        let blocks = parse_blocks("# One\n\nfirst\n\n## Two\n\nsecond\n");
        let mut view = GrammarViewState::default();
        view.layout.ensure(&blocks, 400.0);
        view.layout.update_height(0, 30.0);
        view.layout.update_height(1, 40.0);
        view.layout.update_height(2, 50.0);
        view.layout.rebuild_offsets(&blocks);

        assert_eq!(view.layout.heading_offsets.get(&0).copied(), Some(0.0));
        assert_eq!(
            view.layout.heading_offsets.get(&1).copied(),
            Some(30.0 + super::GRAMMAR_BLOCK_SPACING + 40.0 + super::GRAMMAR_BLOCK_SPACING)
        );
    }

    #[test]
    fn layout_cache_resets_when_wrap_width_changes() {
        let blocks = parse_blocks("# One\n\nfirst paragraph that wraps\n");
        let mut view = GrammarViewState::default();
        view.layout.ensure(&blocks, 400.0);
        view.layout.update_height(0, 123.0);
        view.layout.ensure(&blocks, 400.25);
        assert_eq!(view.layout.block_heights[0], 123.0);

        view.layout.ensure(&blocks, 250.0);
        assert_ne!(view.layout.block_heights[0], 123.0);
        assert_eq!(view.layout.wrap_width, Some(250.0));
        assert_eq!(
            view.layout.block_heights,
            estimated_block_heights(&blocks, 250.0)
        );
    }

    #[test]
    fn info_links_resolve_to_reference_headings() {
        let target = grammar_link_target("#reference-stmtlist.md").expect("stmtlist anchor");
        let heading = grammar_toc()
            .iter()
            .find(|entry| entry.idx == target)
            .expect("target heading");
        assert_eq!(heading.text, "statement-list");
    }

    #[test]
    fn pipe_table_rows_are_not_headings() {
        let blocks = parse_blocks("| Variable Name |: Purpose |\n------------------------------\n");
        assert!(
            blocks
                .iter()
                .all(|block| !matches!(block, Block::Heading { .. })),
            "setext-style table row was parsed as a heading: {blocks:?}"
        );
        assert!(
            blocks
                .iter()
                .any(|block| matches!(block, Block::TableRow(_))),
            "legacy table was not normalized into table rows: {blocks:?}"
        );
        let blocks = parse_blocks(
            "| Variable Name | Initial Value |: Purpose |\n----------------------------------------------\n| arcrad |: 0.250 :| Default arc radius |\n",
        );
        let rows: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::TableRow(cells) => Some(cells),
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), 2, "expected header and data rows: {blocks:?}");
        assert_eq!(
            rows[1].len(),
            3,
            "legacy :| cell markers shifted table columns: {blocks:?}"
        );
        assert!(is_table_row_text(
            "| Legacy ASCII | HTML Entity | Unicode |"
        ));
        assert_eq!(
            normalize_table_row("| arcrad |: 0.250 :| Default arc radius |"),
            "| arcrad | 0.250 | Default arc radius |"
        );
        assert_eq!(
            gfm_table_separator("| Legacy ASCII | HTML Entity | Unicode |"),
            "| --- | --- | --- |"
        );
    }

    #[test]
    fn pikchr_fence_info_is_parsed_into_flags() {
        let blocks = parse_blocks("~~~ pikchr center toggle source\nbox \"A\"\n~~~\n");
        let code = blocks
            .iter()
            .find_map(|block| match block {
                Block::Code(code) => Some(code),
                _ => None,
            })
            .expect("a code block");

        assert_eq!(code.idx, 0);
        assert_eq!(code.info.language.as_deref(), Some("pikchr"));
        assert!(code.info.pikchr);
        assert!(code.info.center);
        assert!(code.info.toggle);
        assert!(code.info.source);
        assert!(!code.info.indent);
        assert_eq!(code.text, "box \"A\"\n");
    }

    #[test]
    fn ordinary_fences_remain_plain_code() {
        let blocks = parse_blocks("~~~ rust\nfn main() {}\n~~~\n");
        let code = blocks
            .iter()
            .find_map(|block| match block {
                Block::Code(code) => Some(code),
                _ => None,
            })
            .expect("a code block");

        assert_eq!(code.info.language.as_deref(), Some("rust"));
        assert!(!code.info.pikchr);
        assert!(!code.info.toggle);
        assert_eq!(code.text, "fn main() {}\n");
    }

    #[test]
    fn code_block_ids_are_stable_and_increment_only_for_code_blocks() {
        let blocks =
            parse_blocks("paragraph\n\n~~~ pikchr\nbox\n~~~\n\n## Heading\n\n~~~\nplain\n~~~\n");
        let ids: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Code(code) => Some(code.idx),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn source_toggle_defaults_are_applied_once_per_block() {
        let block = CodeBlock {
            idx: 7,
            text: "box".into(),
            info: CodeInfo {
                pikchr: true,
                toggle: true,
                source: true,
                ..Default::default()
            },
        };
        let mut view = GrammarViewState::default();

        assert!(code_block_showing_source(&block, &mut view));
        view.source_blocks.remove(&block.idx);
        assert!(
            !code_block_showing_source(&block, &mut view),
            "source default should not be reapplied after the user switches to rendered"
        );
    }

    #[test]
    fn toggle_without_source_defaults_to_rendered() {
        let block = CodeBlock {
            idx: 8,
            text: "box".into(),
            info: CodeInfo {
                pikchr: true,
                toggle: true,
                ..Default::default()
            },
        };
        let mut view = GrammarViewState::default();

        assert!(!code_block_showing_source(&block, &mut view));
    }

    #[test]
    fn toggling_one_block_does_not_affect_another() {
        let mut view = GrammarViewState::default();
        let first = CodeBlock {
            idx: 1,
            text: "box".into(),
            info: CodeInfo {
                pikchr: true,
                toggle: true,
                source: true,
                ..Default::default()
            },
        };
        let second = CodeBlock {
            idx: 2,
            text: "box".into(),
            info: CodeInfo {
                pikchr: true,
                toggle: true,
                ..Default::default()
            },
        };

        assert!(code_block_showing_source(&first, &mut view));
        assert!(!code_block_showing_source(&second, &mut view));
    }

    #[test]
    fn valid_pikchr_block_renders_to_svg_and_image() {
        let block = CodeBlock {
            idx: 0,
            text: "box \"30&deg;\"".into(),
            info: CodeInfo {
                pikchr: true,
                ..Default::default()
            },
        };

        let svg = render_pikchr_svg(&block).expect("valid pikchr should render");
        assert!(svg.contains("<svg"), "missing svg output: {svg}");
        let image = render_pikchr_image(&block, eframe::egui::Color32::WHITE)
            .expect("valid svg should rasterize");
        assert!(image.width() > 0);
        assert!(image.height() > 0);
    }

    #[test]
    fn preview_display_size_uses_texture_scale_with_width_cap() {
        assert_eq!(
            grammar_preview_display_size(eframe::egui::vec2(480.0, 270.0), 1.5, 800.0),
            eframe::egui::vec2(320.0, 180.0)
        );

        let capped = grammar_preview_display_size(eframe::egui::vec2(1500.0, 750.0), 1.5, 800.0);
        assert_eq!(capped.x, 800.0 * super::GRAMMAR_PREVIEW_MAX_WIDTH_FRACTION);
        assert_eq!(capped.y, 320.0);
    }

    #[test]
    fn invalid_pikchr_block_returns_an_error() {
        let block = CodeBlock {
            idx: 0,
            text: "box \"unterminated".into(),
            info: CodeInfo {
                pikchr: true,
                ..Default::default()
            },
        };

        assert!(render_pikchr_svg(&block).is_err());
    }

    #[test]
    fn cached_blocks_match_fresh_parse() {
        assert_eq!(grammar_blocks(), parse_blocks(PIKCHR_GRAMMAR_MD));
    }

    /// Inline `**bold**`/`*italic*`/`` `code` `` must be parsed into spans, so
    /// the literal markers never reach the screen.
    #[test]
    fn inline_markup_is_parsed_into_spans() {
        let blocks = parse_blocks("a **b** c `d` e");
        let para = blocks
            .iter()
            .find_map(|b| match b {
                Block::Para(s) => Some(s),
                _ => None,
            })
            .expect("a paragraph");
        assert!(
            para.iter().any(|s| s.bold && s.text == "b"),
            "missing bold 'b'"
        );
        assert!(
            para.iter().any(|s| s.code && s.text == "d"),
            "missing code 'd'"
        );
        assert!(
            para.iter()
                .all(|s| !s.text.contains("**") && !s.text.contains('`')),
            "literal markup leaked into spans: {para:?}"
        );
    }

    #[test]
    fn html_entities_are_decoded() {
        assert_eq!(decode_entities("a &rarr; b"), "a \u{2192} b");
        assert_eq!(decode_entities("&#9654;"), "\u{25B6}");
        assert_eq!(decode_entities("&#x2192;"), "\u{2192}");
        assert_eq!(decode_entities("a & b"), "a & b");
        assert_eq!(decode_entities("&unknown;"), "&unknown;");
    }
}
