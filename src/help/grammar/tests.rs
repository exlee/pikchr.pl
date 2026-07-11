use super::{
    Block, CodeBlock, CodeInfo, GrammarViewState, PIKCHR_GRAMMAR_MD, Span,
    code_block_showing_source, decode_entities, estimated_block_heights, gfm_table_separator,
    grammar_blocks, grammar_link_target, grammar_preview_display_size, grammar_toc,
    is_table_row_text, normalize_table_row, parse_blocks, render_group_end, render_pikchr_image,
    render_pikchr_svg, table_layout_widths, toc_text, visible_groups,
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
    let blocks =
        parse_blocks("# One\n\nfirst\n\n| A | B |\n| --- | --- |\n| C | D |\n\nsecond\n\n# Two\n");
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
