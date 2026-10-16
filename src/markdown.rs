//! Deterministic Markdown parsing, safe rendering and link indexing.
use crate::model::{Edge, Heading, Link, Metadata, Note, Snapshot, resolve_note};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use std::collections::{BTreeMap, BTreeSet};
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
}

/// Mask parser-identified code/HTML with ASCII spaces, preserving byte offsets and newlines.
pub fn mask_code(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    let mut in_code = false;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        let start_code = matches!(event, Event::Start(Tag::CodeBlock(_)));
        if in_code
            || start_code
            || matches!(
                event,
                Event::Code(_) | Event::Html(_) | Event::InlineHtml(_)
            )
        {
            for byte in &mut bytes[range] {
                if *byte != b'\n' && *byte != b'\r' {
                    *byte = b' ';
                }
            }
        }
        if start_code {
            in_code = true;
        }
        if matches!(event, Event::End(TagEnd::CodeBlock)) {
            in_code = false;
        }
    }
    // Every masked range is on UTF-8 boundaries; spaces replace entire codepoints.
    String::from_utf8(bytes).expect("masking preserves UTF-8")
}

pub fn wiki_links(text: &str) -> Vec<Link> {
    let masked = mask_code(text);
    let mut links = Vec::new();
    let mut offset = 0;
    while let Some(relative) = masked[offset..].find("[[") {
        let start = offset + relative;
        let preceding_slashes = masked[..start]
            .bytes()
            .rev()
            .take_while(|b| *b == b'\\')
            .count();
        if preceding_slashes % 2 == 1 {
            offset = start + 2;
            continue;
        }
        let inner_start = start + 2;
        let Some(close) = masked[inner_start..].find("]]") else {
            break;
        };
        let end = inner_start + close;
        let inner = &text[inner_start..end];
        offset = end + 2;
        if inner.contains(['\n', '\r', '[', ']']) {
            offset = start + 2;
            continue;
        }
        let (destination, alias) = inner
            .split_once('|')
            .map_or((inner, None), |(d, l)| (d, Some(l)));
        let destination = destination.trim();
        let (target, anchor) = destination.split_once('#').unwrap_or((destination, ""));
        let target = target.trim();
        if target.is_empty() && anchor.is_empty() {
            continue;
        }
        let target_start = inner_start + inner.len() - inner.trim_start().len();
        let embed = start > 0 && text.as_bytes()[start - 1] == b'!';
        links.push(Link {
            target: target.into(),
            anchor: anchor.into(),
            label: alias.unwrap_or(destination).into(),
            embed,
            note_id: None,
            range: (start - usize::from(embed))..offset,
            target_range: target_start..target_start + target.len(),
        });
    }
    links
}

fn word_character(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            get_general_category(c),
            GeneralCategory::NonspacingMark
                | GeneralCategory::SpacingMark
                | GeneralCategory::EnclosingMark
        )
        || c == '_'
        || c == '-'
}

pub fn tags(text: &str) -> Vec<String> {
    let masked = mask_code(text);
    let mut found = BTreeSet::new();
    let chars: Vec<(usize, char)> = masked.char_indices().collect();
    for (i, &(offset, c)) in chars.iter().enumerate() {
        if c != '#' || (i > 0 && !chars[i - 1].1.is_whitespace()) {
            continue;
        }
        let mut end = offset + 1;
        for &(pos, value) in &chars[i + 1..] {
            if word_character(value) || value == '/' {
                end = pos + value.len_utf8();
            } else {
                break;
            }
        }
        let tag = &masked[offset + 1..end];
        if !tag.is_empty() && tag.split('/').all(|s| !s.is_empty()) {
            found.insert(tag.nfc().collect());
        }
    }
    found.into_iter().collect()
}

pub fn slug(text: &str) -> String {
    let mut result = String::new();
    let mut space = false;
    for c in text.nfc().flat_map(char::to_lowercase) {
        if c.is_whitespace() {
            space = !result.is_empty();
        } else if word_character(c) {
            if space {
                result.push('-');
                space = false;
            }
            result.push(c);
        }
    }
    if result.is_empty() {
        "section".into()
    } else {
        result
    }
}

pub fn headings(text: &str) -> Vec<Heading> {
    let mut result = Vec::new();
    let mut current: Option<Heading> = None;
    let mut counts = BTreeMap::new();
    let mut used_ids = BTreeSet::new();
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(Heading {
                    level: level as u8,
                    text: String::new(),
                    id: String::new(),
                    index: text[..range.start].encode_utf16().count(),
                })
            }
            Event::Text(value) | Event::Code(value) => {
                if let Some(h) = current.as_mut() {
                    h.text.push_str(&value);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(h) = current.as_mut() {
                    h.text.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(mut heading) = current.take() {
                    let base = slug(&heading.text);
                    let count = counts.entry(base.clone()).or_insert(0usize);
                    loop {
                        heading.id = if *count == 0 {
                            base.clone()
                        } else {
                            format!("{base}-{count}")
                        };
                        *count += 1;
                        if used_ids.insert(heading.id.clone()) {
                            break;
                        }
                    }
                    result.push(heading);
                }
            }
            _ => {}
        }
    }
    result
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn safe_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    !url.chars().any(char::is_control)
        && (lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:"))
}

fn escape_markdown_label(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if "\\`*_{}[]()#+-.!|<>".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn render_markdown(text: &str, links: &[Link], headings: &[Heading]) -> String {
    let mut source = text.to_string();
    for (i, link) in links.iter().enumerate().rev() {
        let replacement = format!("[{}](lattice:{i})", escape_markdown_label(&link.label));
        source.replace_range(link.range.clone(), &replacement);
    }
    let mut output = Vec::new();
    let mut heading_no = 0;
    let mut link_stack = Vec::new();
    for (event, range) in Parser::new_ext(&source, options()).into_offset_iter() {
        match event {
            Event::Html(value) | Event::InlineHtml(value) => output.push(Event::Text(value)),
            Event::Start(Tag::Heading { level, .. }) => {
                let id = headings
                    .get(heading_no)
                    .map_or("section", |h| h.id.as_str());
                output.push(Event::Html(
                    format!("<h{} id=\"{}\">", level as u8, escape(id)).into(),
                ));
                heading_no += 1;
            }
            Event::End(TagEnd::Heading(level)) => {
                output.push(Event::Html(format!("</h{}>", level as u8).into()))
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let generated = dest_url
                    .strip_prefix("lattice:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| links.get(i));
                if let Some(link) = generated {
                    output.push(Event::Html(format!("<a href=\"#\" class=\"wiki-link{}\" data-note=\"{}\" data-id=\"{}\" data-anchor=\"{}\">", if link.note_id.is_some() { "" } else { " missing" }, escape(&link.target), escape(link.note_id.as_deref().unwrap_or_default()), escape(&link.anchor)).into()));
                    link_stack.push(true);
                } else if safe_url(&dest_url) {
                    output.push(Event::Html(
                        format!(
                            "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">",
                            escape(&dest_url)
                        )
                        .into(),
                    ));
                    link_stack.push(true);
                } else {
                    link_stack.push(false);
                }
            }
            Event::End(TagEnd::Link) => {
                if link_stack.pop() == Some(true) {
                    output.push(Event::Html("</a>".into()));
                }
            }
            // Display image alt text; never request user-supplied remote URLs.
            Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => {}
            Event::TaskListMarker(checked) => {
                let line = source[..range.start]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count();
                output.push(Event::Html(format!("<input type=\"checkbox\" aria-label=\"Toggle task\" data-line=\"{line}\" {}>", if checked { "checked" } else { "" }).into()));
            }
            other => output.push(other),
        }
    }
    let mut html = String::new();
    html::push_html(&mut html, output.into_iter());
    html
}

pub fn snapshot(revision: i64, notes: Vec<Note>) -> Snapshot {
    let mut metadata = BTreeMap::new();
    let mut backlinks: BTreeMap<String, Vec<String>> =
        notes.iter().map(|n| (n.id.clone(), Vec::new())).collect();
    let mut edges = Vec::new();
    for note in &notes {
        let mut links = wiki_links(&note.body);
        let mut destinations = BTreeSet::new();
        for link in &mut links {
            link.note_id = if link.target.is_empty() {
                Some(note.id.clone())
            } else {
                resolve_note(&notes, &link.target).map(|n| n.id.clone())
            };
            if let Some(id) = &link.note_id {
                destinations.insert(id.clone());
            }
        }
        for target in destinations {
            if let Some(incoming) = backlinks.get_mut(&target) {
                incoming.push(note.id.clone());
            }
            if target != note.id {
                edges.push(Edge {
                    source: note.id.clone(),
                    target,
                });
            }
        }
        let headings = headings(&note.body);
        let html = render_markdown(&note.body, &links, &headings);
        metadata.insert(
            note.id.clone(),
            Metadata {
                links,
                headings,
                html,
                tags: tags(&note.body),
            },
        );
    }
    Snapshot {
        notebook_id: String::new(),
        revision,
        notes,
        metadata,
        backlinks,
        edges,
    }
}

pub fn search(snapshot: &Snapshot, query: &str, tag: &str) -> Vec<String> {
    let query: String = query.to_lowercase().nfc().collect();
    let terms: Vec<&str> = query.split_whitespace().collect();
    let tag: String = tag.nfc().collect();
    let mut found: Vec<&Note> = snapshot
        .notes
        .iter()
        .filter(|n| {
            let haystack: String = format!("{}\n{}", n.title, n.body)
                .to_lowercase()
                .nfc()
                .collect();
            terms.iter().all(|term| haystack.contains(term))
                && (tag.is_empty()
                    || snapshot
                        .metadata
                        .get(&n.id)
                        .is_some_and(|m| m.tags.contains(&tag)))
        })
        .collect();
    found.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
    found.into_iter().map(|n| n.id.clone()).collect()
}

/// Rewrite only links resolving to these original identities, preserving aliases and anchors.
pub fn rewrite_targets(
    body: &str,
    original: &[Note],
    replacements: &BTreeMap<String, String>,
) -> String {
    let mut result = body.to_string();
    for link in wiki_links(body).into_iter().rev() {
        if link.target.is_empty() {
            continue;
        }
        if let Some(title) =
            resolve_note(original, &link.target).and_then(|n| replacements.get(&n.id))
        {
            let replacement = if link.target.to_ascii_lowercase().ends_with(".md")
                && !title.to_ascii_lowercase().ends_with(".md")
            {
                format!("{title}.md")
            } else {
                title.clone()
            };
            result.replace_range(link.target_range, &replacement);
        }
    }
    result
}
