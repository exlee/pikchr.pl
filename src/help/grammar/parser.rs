use super::*;

pub(super) fn heading_level(l: pulldown_cmark::HeadingLevel) -> u8 {
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
pub(super) fn parse_doc(src: &str) -> GrammarDoc {
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
pub(super) fn parse_blocks(src: &str) -> Vec<Block> {
    parse_doc(src).blocks
}

pub(super) fn parse_code_info(info: &str) -> CodeInfo {
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

pub(super) fn normalize_doc_tables(src: &str) -> String {
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

pub(super) fn normalize_table_row(row: &str) -> String {
    decode_entities(row)
        .replace('\u{00A0}', " ")
        .replace("|:", "|")
        .replace(":|", "|")
}

pub(super) fn is_legacy_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '-')
}

pub(super) fn gfm_table_separator(header: &str) -> String {
    let columns = pipe_column_count(header).max(1);
    let mut out = String::new();
    out.push('|');
    for _ in 0..columns {
        out.push_str(" --- |");
    }
    out
}

pub(super) fn pipe_column_count(row: &str) -> usize {
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
pub(super) fn push_span(
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
pub(super) fn decode_entities(input: &str) -> String {
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

pub(super) fn html_to_text(input: &str) -> String {
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

pub(super) fn extract_anchor_ids(input: &str) -> Vec<String> {
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
