use regex::Regex;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const ERR_SORT_FIELD_REQUIRED: &str = "sort_field_required";
const ERR_TABLE_COLUMNS_REQUIRED: &str = "table_columns_required";
const ERR_UNSUPPORTED_QUERY: &str = "unsupported_query";
const ERR_WHERE_COMPARISON_REQUIRED: &str = "where_comparison_required";

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DataviewResult {
    pub kind: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub items: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct Page {
    path: String,
    title: String,
    fields: BTreeMap<String, String>,
    tags: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct Column {
    field: String,
    label: String,
}

#[derive(Debug, Clone)]
enum QueryKind {
    Table(Vec<Column>),
    List(Option<String>),
}

#[derive(Debug, Clone)]
struct Query {
    kind: QueryKind,
    source: Option<Source>,
    filter: Option<Filter>,
    sort: Option<(String, bool)>,
    limit: Option<usize>,
}

#[derive(Debug, Clone)]
enum Source {
    Tag(String),
    Folder(String),
}

#[derive(Debug, Clone)]
struct Filter {
    field: String,
    op: FilterOp,
    value: String,
}

#[derive(Debug, Clone)]
enum FilterOp {
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
}

pub fn query_dataview(vault_root: &Path, query: &str) -> std::io::Result<DataviewResult> {
    let parsed = match parse_query(query) {
        Ok(q) => q,
        Err(e) => return Ok(error_result(e)),
    };
    let mut pages = index_pages(vault_root)?;
    if let Some(source) = &parsed.source {
        pages.retain(|p| matches_source(p, source));
    }
    if let Some(filter) = &parsed.filter {
        pages.retain(|p| matches_filter(p, filter));
    }
    if let Some((field, desc)) = &parsed.sort {
        pages.sort_by(|a, b| compare_field(a, b, field));
        if *desc {
            pages.reverse();
        }
    }
    if let Some(limit) = parsed.limit {
        pages.truncate(limit);
    }

    Ok(match parsed.kind {
        QueryKind::Table(columns) => DataviewResult {
            kind: "table".into(),
            columns: columns.iter().map(|c| c.label.clone()).collect(),
            rows: pages
                .iter()
                .map(|p| columns.iter().map(|c| field_value(p, &c.field)).collect())
                .collect(),
            items: Vec::new(),
            error: None,
        },
        QueryKind::List(field) => DataviewResult {
            kind: "list".into(),
            columns: Vec::new(),
            rows: Vec::new(),
            items: pages
                .iter()
                .map(|p| {
                    field
                        .as_ref()
                        .map(|f| field_value(p, f))
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| format!("[[{}]]", stem(&p.path)))
                })
                .collect(),
            error: None,
        },
    })
}

fn error_result(message: String) -> DataviewResult {
    DataviewResult {
        kind: "error".into(),
        columns: Vec::new(),
        rows: Vec::new(),
        items: Vec::new(),
        error: Some(message),
    }
}

fn parse_query(raw: &str) -> Result<Query, String> {
    let mut first = String::new();
    let mut source = None;
    let mut filter = None;
    let mut sort = None;
    let mut limit = None;

    for line in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if first.is_empty() {
            if let Some((before, after)) = split_first_clause(line) {
                first = before.trim().to_string();
                parse_clauses(after, &mut source, &mut filter, &mut sort, &mut limit)?;
            } else {
                first = line.to_string();
            }
            continue;
        }
        parse_clauses(line, &mut source, &mut filter, &mut sort, &mut limit)?;
    }

    let kind = if let Some(rest) = strip_prefix_ci(&first, "table ") {
        let cols = rest
            .split(',')
            .map(parse_column)
            .filter(|c| !c.field.is_empty())
            .collect::<Vec<_>>();
        if cols.is_empty() {
            return Err(ERR_TABLE_COLUMNS_REQUIRED.into());
        }
        QueryKind::Table(cols)
    } else if first.eq_ignore_ascii_case("list") {
        QueryKind::List(None)
    } else if let Some(rest) = strip_prefix_ci(&first, "list ") {
        QueryKind::List(Some(rest.trim().to_string()))
    } else {
        return Err(ERR_UNSUPPORTED_QUERY.into());
    };

    Ok(Query {
        kind,
        source,
        filter,
        sort,
        limit,
    })
}

fn parse_clauses(
    mut text: &str,
    source: &mut Option<Source>,
    filter: &mut Option<Filter>,
    sort: &mut Option<(String, bool)>,
    limit: &mut Option<usize>,
) -> Result<(), String> {
    while let Some((keyword, body, rest)) = split_leading_clause(text) {
        match keyword {
            "from" => *source = parse_source(body),
            "where" => *filter = Some(parse_filter(body)?),
            "sort" => *sort = Some(parse_sort(body)?),
            "limit" => *limit = body.trim().parse::<usize>().ok(),
            _ => {}
        }
        text = rest;
    }
    Ok(())
}

fn parse_sort(text: &str) -> Result<(String, bool), String> {
    let mut parts = text.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(ERR_SORT_FIELD_REQUIRED.into());
    }
    let desc = parts
        .last()
        .map(|p| p.eq_ignore_ascii_case("desc"))
        .unwrap_or(false);
    if matches!(parts.last(), Some(p) if p.eq_ignore_ascii_case("asc") || p.eq_ignore_ascii_case("desc"))
    {
        parts.pop();
    }
    Ok((parts.join(" "), desc))
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .and_then(|_| text.get(prefix.len()..))
}

fn split_first_clause(text: &str) -> Option<(&str, &str)> {
    find_clause_start(text).and_then(|idx| Some((text.get(..idx)?, text.get(idx..)?)))
}

fn split_leading_clause(text: &str) -> Option<(&'static str, &str, &str)> {
    let text = text.trim_start();
    let (keyword, body_start) = leading_clause_keyword(text)?;
    let body = text.get(body_start..)?.trim_start();
    if let Some(next) = find_clause_start(body) {
        Some((keyword, body.get(..next)?.trim(), body.get(next..)?))
    } else {
        Some((keyword, body.trim(), ""))
    }
}

fn leading_clause_keyword(text: &str) -> Option<(&'static str, usize)> {
    for keyword in ["from", "where", "sort", "limit"] {
        let prefix = format!("{keyword} ");
        if let Some(rest) = strip_prefix_ci(text, &prefix) {
            return Some((keyword, text.len() - rest.len()));
        }
    }
    None
}

fn find_clause_start(text: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (idx, ch) in text.char_indices() {
        if matches!(ch, '"' | '\'') {
            quote = match quote {
                Some(q) if q == ch => None,
                None => Some(ch),
                other => other,
            };
            continue;
        }
        if quote.is_some() {
            continue;
        }
        if idx > 0
            && !text[..idx]
                .chars()
                .next_back()
                .map(|c| c.is_whitespace())
                .unwrap_or(false)
        {
            continue;
        }
        if leading_clause_keyword(&text[idx..]).is_some() {
            return Some(idx);
        }
    }
    None
}

fn parse_source(text: &str) -> Option<Source> {
    let trimmed = text.trim();
    if let Some(tag) = trimmed.strip_prefix('#') {
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches(',')
            .trim_matches('#');
        if !name.is_empty() {
            return Some(Source::Tag(name.to_ascii_lowercase()));
        }
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return Some(Source::Folder(rest[..end].trim_matches('/').to_string()));
        }
    }
    None
}

fn parse_filter(text: &str) -> Result<Filter, String> {
    for (needle, op) in [
        (">=", FilterOp::Gte),
        ("<=", FilterOp::Lte),
        ("!=", FilterOp::NotEq),
        ("=", FilterOp::Eq),
        (">", FilterOp::Gt),
        ("<", FilterOp::Lt),
    ] {
        if let Some((field, value)) = text.split_once(needle) {
            return Ok(Filter {
                field: field.trim().to_string(),
                op,
                value: clean_value(value),
            });
        }
    }
    Err(ERR_WHERE_COMPARISON_REQUIRED.into())
}

fn parse_column(raw: &str) -> Column {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(idx) = lower.find(" as ") {
        Column {
            field: trimmed[..idx].trim().to_string(),
            label: trimmed[idx + 4..].trim().trim_matches('"').to_string(),
        }
    } else {
        Column {
            field: trimmed.to_string(),
            label: trimmed.to_string(),
        }
    }
}

fn index_pages(vault_root: &Path) -> std::io::Result<Vec<Page>> {
    let mut pages = Vec::new();
    collect_pages(vault_root, vault_root, &mut pages)?;
    Ok(pages)
}

fn collect_pages(root: &Path, dir: &Path, pages: &mut Vec<Page>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_pages(root, &path, pages)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let title = crate::vault::title_of(&path, &content);
            let (frontmatter, body) = split_frontmatter(&content);
            let mut fields = BTreeMap::new();
            fields.insert("file.name".into(), stem(&rel).to_string());
            fields.insert("file.path".into(), rel.clone());
            fields.insert("file.link".into(), format!("[[{}]]", stem(&rel)));
            fields.insert("title".into(), title.clone());
            let mut tags = BTreeSet::new();
            parse_frontmatter(frontmatter, &mut fields, &mut tags);
            parse_inline_fields(body, &mut fields);
            parse_tags(body, &mut tags);
            pages.push(Page {
                path: rel,
                title,
                fields,
                tags,
            });
        }
    }
    Ok(())
}

fn split_frontmatter(content: &str) -> (&str, &str) {
    if !content.starts_with("---") {
        return ("", content);
    }
    let Some(close) = content[3..].find("\n---") else {
        return ("", content);
    };
    let end = close + 3;
    let after = content[end + 4..]
        .strip_prefix('\n')
        .unwrap_or(&content[end + 4..]);
    (&content[3..end], after)
}

fn parse_frontmatter(fm: &str, fields: &mut BTreeMap<String, String>, tags: &mut BTreeSet<String>) {
    let mut current_list: Option<String> = None;
    for line in fm.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(key) = &current_list {
            if let Some(item) = trimmed.strip_prefix('-') {
                let item = clean_value(item);
                append_field(fields, key, &item);
                if key == "tags" {
                    add_tag(tags, &item);
                }
                continue;
            }
        }
        current_list = None;
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() {
            current_list = Some(key);
            continue;
        }
        let value = clean_value(value);
        fields.insert(key.clone(), value.clone());
        if key == "tags" {
            for tag in value.split(',') {
                add_tag(tags, tag);
            }
        }
    }
}

fn parse_inline_fields(body: &str, fields: &mut BTreeMap<String, String>) {
    let Ok(re) = Regex::new(r"(?m)(?:^|\[|\s)([A-Za-z][A-Za-z0-9 _-]{0,60})::\s*([^\]\n]+)") else {
        return;
    };
    for cap in re.captures_iter(body) {
        let key = cap[1].trim().to_ascii_lowercase();
        let value = clean_value(cap[2].trim());
        if !key.is_empty() && !value.is_empty() {
            fields.insert(key, value);
        }
    }
}

fn parse_tags(body: &str, tags: &mut BTreeSet<String>) {
    let Ok(re) = Regex::new(r"(?:^|\s)#([A-Za-z0-9_/-]+)") else {
        return;
    };
    for cap in re.captures_iter(body) {
        add_tag(tags, &cap[1]);
    }
}

fn clean_value(value: &str) -> String {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return trimmed[1..trimmed.len() - 1]
            .split(',')
            .map(|v| v.trim().trim_matches('"').trim_matches('\''))
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    trimmed.to_string()
}

fn append_field(fields: &mut BTreeMap<String, String>, key: &str, value: &str) {
    fields
        .entry(key.to_string())
        .and_modify(|existing| {
            if !existing.is_empty() {
                existing.push_str(", ");
            }
            existing.push_str(value);
        })
        .or_insert_with(|| value.to_string());
}

fn add_tag(tags: &mut BTreeSet<String>, tag: &str) {
    let cleaned = tag
        .trim()
        .trim_start_matches('#')
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase();
    if !cleaned.is_empty() {
        tags.insert(cleaned);
    }
}

fn matches_source(page: &Page, source: &Source) -> bool {
    match source {
        Source::Tag(tag) => page
            .tags
            .iter()
            .any(|t| t == tag || t.starts_with(&format!("{tag}/"))),
        Source::Folder(folder) => {
            folder.is_empty()
                || page.path == *folder
                || page.path.starts_with(&format!("{folder}/"))
        }
    }
}

fn matches_filter(page: &Page, filter: &Filter) -> bool {
    let actual = field_value(page, &filter.field);
    let expected = filter.value.clone();
    match (actual.parse::<f64>(), expected.parse::<f64>()) {
        (Ok(a), Ok(b)) => match filter.op {
            FilterOp::Eq => (a - b).abs() < f64::EPSILON,
            FilterOp::NotEq => (a - b).abs() >= f64::EPSILON,
            FilterOp::Gt => a > b,
            FilterOp::Gte => a >= b,
            FilterOp::Lt => a < b,
            FilterOp::Lte => a <= b,
        },
        _ => {
            let a = actual.to_lowercase();
            let b = expected.to_lowercase();
            match filter.op {
                FilterOp::Eq => a == b,
                FilterOp::NotEq => a != b,
                FilterOp::Gt => a > b,
                FilterOp::Gte => a >= b,
                FilterOp::Lt => a < b,
                FilterOp::Lte => a <= b,
            }
        }
    }
}

fn compare_field(a: &Page, b: &Page, field: &str) -> Ordering {
    let av = field_value(a, field);
    let bv = field_value(b, field);
    match (av.parse::<f64>(), bv.parse::<f64>()) {
        (Ok(an), Ok(bn)) => an.partial_cmp(&bn).unwrap_or(Ordering::Equal),
        _ => av.to_lowercase().cmp(&bv.to_lowercase()),
    }
}

fn field_value(page: &Page, field: &str) -> String {
    let key = field.trim().to_ascii_lowercase();
    if key == "file.name" {
        return stem(&page.path).to_string();
    }
    if key == "file.path" {
        return page.path.clone();
    }
    if key == "file.link" {
        return format!("[[{}]]", stem(&page.path));
    }
    if key == "title" {
        return page.title.clone();
    }
    page.fields.get(&key).cloned().unwrap_or_default()
}

fn stem(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .strip_suffix(".md")
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path))
}

#[cfg(test)]
mod tests {
    use crate::vault;

    use super::*;

    fn tmp_vault() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let uniq = format!(
            "magma-dataview-test-{:?}-{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        p.push(uniq);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn table_reads_frontmatter_and_inline_fields() {
        let v = tmp_vault();
        vault::write_note(
            &v,
            "books/Dune.md",
            "---\ntags: [book]\nrating: 9\n---\n# Dune\nAuthor:: Frank Herbert",
        )
        .unwrap();
        vault::write_note(
            &v,
            "books/Other.md",
            "---\ntags: [book]\nrating: 5\n---\n# Other",
        )
        .unwrap();

        let result = query_dataview(
            &v,
            "TABLE file.link AS \"Name\", rating, author\nFROM #book\nWHERE rating >= 8\nSORT rating DESC",
        )
        .unwrap();
        assert_eq!(result.kind, "table");
        assert_eq!(result.columns, vec!["Name", "rating", "author"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0], vec!["[[Dune]]", "9", "Frank Herbert"]);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn list_filters_by_folder_and_limits() {
        let v = tmp_vault();
        vault::write_note(&v, "projects/A.md", "# A").unwrap();
        vault::write_note(&v, "projects/B.md", "# B").unwrap();
        vault::write_note(&v, "archive/C.md", "# C").unwrap();

        let result = query_dataview(&v, "LIST\nFROM \"projects\"\nLIMIT 1").unwrap();
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0] == "[[A]]" || result.items[0] == "[[B]]");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn single_line_query_applies_where_sort_and_limit() {
        let v = tmp_vault();
        vault::write_note(&v, "A.md", "---\ntags: [x]\nrating: 9\n---\n# A").unwrap();
        vault::write_note(&v, "B.md", "---\ntags: [x]\nrating: 5\n---\n# B").unwrap();
        vault::write_note(&v, "C.md", "---\ntags: [x]\nrating: 2\n---\n# C").unwrap();

        let filtered = query_dataview(&v, "TABLE rating FROM #x WHERE rating >= 8").unwrap();
        assert_eq!(filtered.rows, vec![vec!["9"]]);

        let limited = query_dataview(&v, "LIST FROM #x LIMIT 1").unwrap();
        assert_eq!(limited.items.len(), 1);

        let sorted = query_dataview(&v, "TABLE rating FROM #x SORT rating DESC").unwrap();
        assert_eq!(sorted.rows, vec![vec!["9"], vec!["5"], vec!["2"]]);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn non_ascii_fields_do_not_panic() {
        let v = tmp_vault();
        vault::write_note(
            &v,
            "bücher/Überblick.md",
            "---\ntags: [bücher]\nqualität: 10\n---\n# Überblick",
        )
        .unwrap();

        let result = query_dataview(
            &v,
            "TABLE file.link, qualität\nFROM \"bücher\"\nWHERE qualität >= 9",
        )
        .unwrap();
        assert_eq!(result.rows, vec![vec!["[[Überblick]]", "10"]]);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn parser_errors_are_stable_codes() {
        let v = tmp_vault();
        let result = query_dataview(&v, "TASK").unwrap();
        assert_eq!(result.error.as_deref(), Some(ERR_UNSUPPORTED_QUERY));
        fs::remove_dir_all(&v).ok();
    }
}
