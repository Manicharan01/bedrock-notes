use lattice::{markdown::*, model::*};

fn note(id: &str, title: &str, body: &str) -> Note {
    Note {
        id: id.into(),
        title: title.into(),
        body: body.into(),
        updated: 1,
    }
}

#[test]
fn aliases_anchors_embeddings_unicode_and_deduplicated_backlinks() {
    let s = snapshot(
        0,
        vec![
            note(
                "a",
                "Alpha",
                "[[Beta]] [[Beta#Part|B]] ![[Beta]] [[Missing]]",
            ),
            note("b", "Beta", "# Part"),
        ],
    );
    let m = &s.metadata["a"];
    assert_eq!(m.links.len(), 4);
    assert_eq!(m.links[1].anchor, "Part");
    assert_eq!(m.links[1].label, "B");
    assert!(m.links[2].embed);
    assert_eq!(s.backlinks["b"], vec!["a"]);
    assert_eq!(s.edges.len(), 1);
    assert!(m.links[3].note_id.is_none());
    assert!(m.html.contains("data-id=\"b\""));
    assert!(m.html.contains("data-anchor=\"Part\""));
}

#[test]
fn code_fences_variable_delimiters_inline_code_and_unclosed_code_are_not_indexed() {
    let text = "[[Real]] #real\n````md\n[[Fake]] #fake\n`````\n``[[Inline]] ` #inline``\n\n    [[Indented]] #hidden\n\n~~~\n[[Unclosed]] #hidden";
    assert_eq!(
        wiki_links(text)
            .iter()
            .map(|l| l.target.as_str())
            .collect::<Vec<_>>(),
        vec!["Real"]
    );
    assert_eq!(tags(text), vec!["real"]);
}

#[test]
fn escaped_links_and_html_are_not_indexed() {
    let text = "\\[[Escaped]] [[Real]]\n\n<!-- [[Comment]] -->\n\n<div>[[HTML]]</div>";
    assert_eq!(
        wiki_links(text)
            .iter()
            .map(|l| l.target.as_str())
            .collect::<Vec<_>>(),
        vec!["Real"]
    );
}

#[test]
fn malformed_link_does_not_swallow_a_later_valid_link() {
    assert_eq!(
        wiki_links("[[broken\n[[Good]]")
            .iter()
            .map(|l| l.target.as_str())
            .collect::<Vec<_>>(),
        vec!["Good"]
    );
}

#[test]
fn paragraph_continuation_indentation_is_not_an_indented_code_block() {
    assert_eq!(wiki_links("paragraph\n    [[Valid]]").len(), 1);
}

#[test]
fn basename_ambiguity_and_nfc_resolution() {
    let notes = vec![
        note("a", "a/Note", ""),
        note("b", "b/Note", ""),
        note("c", "Café", ""),
    ];
    assert!(resolve_note(&notes, "Note").is_none());
    assert_eq!(resolve_note(&notes, "a/NOTE.md").unwrap().id, "a");
    assert_eq!(resolve_note(&notes, "Cafe\u{301}").unwrap().id, "c");
}

#[test]
fn self_heading_link_uses_current_identity() {
    let s = snapshot(0, vec![note("a", "Alpha", "# Part\n\n[[#Part]]")]);
    assert_eq!(s.metadata["a"].links[0].note_id.as_deref(), Some("a"));
    assert!(s.edges.is_empty());
}

#[test]
fn rename_rewrites_only_resolved_target_spans_and_preserves_code_aliases_and_extensions() {
    let original = vec![note("b", "Folder/Beta", "")];
    let replacements = std::collections::BTreeMap::from([("b".into(), "新しい😀".into())]);
    let text = "😀 [[  Beta.md#Part|label ]] [[Folder/Beta]] `[[Beta]]` [[Missing]]";
    assert_eq!(
        rewrite_targets(text, &original, &replacements),
        "😀 [[  新しい😀.md#Part|label ]] [[新しい😀]] `[[Beta]]` [[Missing]]"
    );
}

#[test]
fn rename_to_md_title_does_not_duplicate_extension() {
    let original = vec![note("a", "Alpha", "")];
    let r = std::collections::BTreeMap::from([("a".into(), "Beta.md".into())]);
    assert_eq!(
        rewrite_targets("[[Alpha.md]]", &original, &r),
        "[[Beta.md]]"
    );
}

#[test]
fn raw_html_and_unsafe_links_are_escaped_or_removed() {
    let input = "<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n[x](javascript:alert%281%29) [x](data:text/html,test) ![alt](https://example.com/tracker) [safe](https://example.com)";
    let s = snapshot(0, vec![note("a", "Alpha", input)]);
    let html = &s.metadata["a"].html;
    assert!(!html.contains("<script"));
    assert!(!html.contains("<img"));
    assert!(!html.contains("href=\"javascript:"));
    assert!(!html.contains("href=\"data:"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("rel=\"noopener noreferrer\""));
}

#[test]
fn wiki_labels_and_attributes_cannot_inject_html() {
    let s = snapshot(
        0,
        vec![note(
            "a",
            "Alpha",
            "[[\" onmouseover=bad|<img src=x onerror=bad>]]",
        )],
    );
    let html = &s.metadata["a"].html;
    assert!(!html.contains("<img"));
    assert!(!html.contains("data-note=\"\" onmouseover"));
    assert!(html.contains("&quot;"));
}

#[test]
fn heading_offsets_match_utf16_and_ids_are_unique_even_with_suffix_collisions() {
    let text = "😀 hi\n\n# A\n# A\n# A-1\n# A";
    let h = headings(text);
    assert_eq!(h[0].index, 7);
    let ids: std::collections::HashSet<_> = h.iter().map(|h| &h.id).collect();
    assert_eq!(ids.len(), 4);
    let s = snapshot(0, vec![note("a", "Alpha", text)]);
    for h in &h {
        assert!(s.metadata["a"].html.contains(&format!("id=\"{}\"", h.id)));
    }
}

#[test]
fn unicode_tags_and_normalized_search() {
    let s = snapshot(
        0,
        vec![
            note("a", "Café", "#தமிழ் #topics/rust\n\ntext `#code`"),
            note("b", "Other", "#other"),
        ],
    );
    assert_eq!(search(&s, "CAFE\u{301} text", "தமிழ்"), vec!["a"]);
    assert_eq!(s.metadata["a"].tags.len(), 2);
    assert!(search(&s, "cafe missing", "").is_empty());
}

#[test]
fn tasks_keep_line_numbers_after_unicode_and_wiki_expansion() {
    let s = snapshot(
        0,
        vec![note(
            "a",
            "Alpha",
            "[[Alpha|A label much longer than the target]]\r\n\r\n- [ ] Task\r\n- [x] Done",
        )],
    );
    let html = &s.metadata["a"].html;
    assert!(html.contains("data-line=\"2\""));
    assert!(html.contains("data-line=\"3\" checked"));
}

#[test]
fn malformed_unicode_markdown_does_not_panic() {
    for body in [
        "😀[[",
        "[[#]]",
        "[[東京|😀]]",
        "`😀[[bad]]",
        "~~~\n[[",
        "[[\u{2003}東京.md #h|x]]",
        "\0\0",
        "[[a\nb]]",
        "#[[]]",
        "![[é]]",
    ] {
        let s = snapshot(0, vec![note("a", "東京", body)]);
        assert!(s.metadata.contains_key("a"));
    }
}

#[test]
fn traversal_duplicate_id_empty_and_oversized_title_validation() {
    for bad in [
        "",
        "../secret",
        "a//b",
        "a/../b",
        "a\\b",
        "x:y",
        "Folder/ note",
        ".md",
        "a\n",
    ] {
        assert!(validate_title(bad).is_err(), "{bad:?}");
    }
    assert!(validate_title(&"😀".repeat(180)).is_ok());
    assert!(validate_title(&"😀".repeat(181)).is_err());
    assert!(validate_notes(&[note("a", "Alpha", ""), note("a", "Beta", "")]).is_err());
}
