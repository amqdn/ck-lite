use anyhow::Result;
use ck_core::{CkError, IncludePattern, SearchOptions, SearchResult, Span};
use globset::{Glob, GlobSet, GlobSetBuilder};
use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn regex_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    let pattern = if options.fixed_string {
        regex::escape(&options.query)
    } else if options.whole_word {
        format!(r"\b{}\b", regex::escape(&options.query))
    } else {
        options.query.clone()
    };

    let regex = RegexBuilder::new(&pattern)
        .case_insensitive(options.case_insensitive)
        .build()
        .map_err(CkError::Regex)?;

    let should_recurse = options.path.is_dir() || options.recursive;
    let files = if should_recurse {
        let file_options = ck_core::FileCollectionOptions {
            respect_gitignore: options.respect_gitignore,
            use_ckignore: options.use_ckignore,
            exclude_patterns: options.exclude_patterns.clone(),
            show_hidden: options.hidden,
        };
        let collected = ck_index::collect_files(&options.path, &file_options)?;
        filter_files_by_include(collected, &options.include_patterns)
    } else {
        let mut exclude_patterns = options.exclude_patterns.clone();
        if !(options.respect_gitignore && ck_core::has_gitignore(&options.path)) {
            exclude_patterns.extend(ck_core::get_gitignore_fallback_patterns());
        }
        let collected = collect_files(&options.path, should_recurse, &exclude_patterns)?;
        filter_files_by_include(collected, &options.include_patterns)
    };

    let results: Vec<Vec<SearchResult>> = files
        .par_iter()
        .filter_map(|file_path| match search_file(&regex, file_path, options) {
            Ok(matches) => {
                if matches.is_empty() {
                    None
                } else {
                    Some(matches)
                }
            }
            Err(e) => {
                tracing::debug!("Error searching {:?}: {}", file_path, e);
                None
            }
        })
        .collect();

    let mut all_results: Vec<SearchResult> = results.into_iter().flatten().collect();

    all_results.sort_by(|a, b| {
        let path_cmp = a.file.cmp(&b.file);
        if path_cmp != std::cmp::Ordering::Equal {
            return path_cmp;
        }
        a.span.line_start.cmp(&b.span.line_start)
    });

    if let Some(top_k) = options.top_k {
        all_results.truncate(top_k);
    }

    Ok(all_results)
}

fn search_file(
    regex: &Regex,
    file_path: &Path,
    options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    if options.context_lines > 0
        || options.before_context_lines > 0
        || options.after_context_lines > 0
    {
        let content = fs::read_to_string(file_path)?;
        let (lines, line_ending_lengths) = split_lines_with_endings(&content);
        search_file_in_memory(regex, file_path, options, &lines, &line_ending_lengths)
    } else {
        search_file_streaming(regex, file_path, options)
    }
}

fn search_file_in_memory(
    regex: &Regex,
    file_path: &Path,
    options: &SearchOptions,
    lines: &[String],
    line_ending_lengths: &[usize],
) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let mut byte_offset = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;

        if regex.as_str().is_empty() {
            let preview = get_context_preview(lines, line_idx, options);
            results.push(SearchResult {
                file: file_path.to_path_buf(),
                span: Span {
                    byte_start: byte_offset,
                    byte_end: byte_offset + line.len(),
                    line_start: line_number,
                    line_end: line_number,
                },
                score: 1.0,
                preview,
                lang: ck_core::Language::from_path(file_path),
                symbol: None,
            });
        } else {
            for mat in regex.find_iter(line) {
                let preview = get_context_preview(lines, line_idx, options);
                results.push(SearchResult {
                    file: file_path.to_path_buf(),
                    span: Span {
                        byte_start: byte_offset + mat.start(),
                        byte_end: byte_offset + mat.end(),
                        line_start: line_number,
                        line_end: line_number,
                    },
                    score: 1.0,
                    preview,
                    lang: ck_core::Language::from_path(file_path),
                    symbol: None,
                });
            }
        }

        byte_offset += line.len();
        byte_offset += line_ending_lengths.get(line_idx).copied().unwrap_or(0);
    }

    Ok(results)
}

fn search_file_streaming(
    regex: &Regex,
    file_path: &Path,
    _options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    use std::io::{BufRead, BufReader};

    let file = fs::File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let mut results = Vec::new();
    let mut line = String::new();
    let mut byte_offset = 0usize;
    let mut line_number = 1usize;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let mut newline_len = 0usize;
        if line.ends_with("\r\n") {
            line.pop();
            line.pop();
            newline_len = 2;
        } else if line.ends_with(['\n', '\r']) {
            line.pop();
            newline_len = 1;
        }

        let treat_cr_as_newline = line.contains('\r');

        if treat_cr_as_newline {
            let bytes = line.as_bytes();
            let mut segment_start = 0usize;
            while segment_start <= bytes.len() {
                match bytes[segment_start..].iter().position(|&b| b == b'\r') {
                    Some(rel_idx) => {
                        let idx = segment_start + rel_idx;
                        let segment_bytes = &bytes[segment_start..idx];
                        let segment_str = std::str::from_utf8(segment_bytes)?;
                        process_streaming_line(
                            regex,
                            file_path,
                            segment_str,
                            line_number,
                            byte_offset,
                            &mut results,
                        );
                        byte_offset += segment_bytes.len() + 1;
                        line_number += 1;
                        segment_start = idx + 1;
                    }
                    None => {
                        let segment_bytes = &bytes[segment_start..];
                        let segment_str = std::str::from_utf8(segment_bytes)?;
                        process_streaming_line(
                            regex,
                            file_path,
                            segment_str,
                            line_number,
                            byte_offset,
                            &mut results,
                        );
                        byte_offset += segment_bytes.len();
                        line_number += 1;
                        break;
                    }
                }
            }
            byte_offset += newline_len;
        } else {
            let line_str = line.as_str();
            process_streaming_line(
                regex,
                file_path,
                line_str,
                line_number,
                byte_offset,
                &mut results,
            );
            byte_offset += line_str.len() + newline_len;
            line_number += 1;
        }
    }

    Ok(results)
}

fn process_streaming_line(
    regex: &Regex,
    file_path: &Path,
    line: &str,
    line_number: usize,
    byte_offset: usize,
    results: &mut Vec<SearchResult>,
) {
    if regex.as_str().is_empty() {
        results.push(SearchResult {
            file: file_path.to_path_buf(),
            span: Span {
                byte_start: byte_offset,
                byte_end: byte_offset + line.len(),
                line_start: line_number,
                line_end: line_number,
            },
            score: 1.0,
            preview: line.to_string(),
            lang: ck_core::Language::from_path(file_path),
            symbol: None,
        });
    } else {
        for mat in regex.find_iter(line) {
            results.push(SearchResult {
                file: file_path.to_path_buf(),
                span: Span {
                    byte_start: byte_offset + mat.start(),
                    byte_end: byte_offset + mat.end(),
                    line_start: line_number,
                    line_end: line_number,
                },
                score: 1.0,
                preview: line.to_string(),
                lang: ck_core::Language::from_path(file_path),
                symbol: None,
            });
        }
    }
}

fn split_lines_with_endings(content: &str) -> (Vec<String>, Vec<usize>) {
    let mut lines = Vec::new();
    let mut endings = Vec::new();

    let bytes = content.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                lines.push(content[start..i].to_string());
                endings.push(1);
                i += 1;
                start = i;
            }
            b'\r' => {
                lines.push(content[start..i].to_string());
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    endings.push(2);
                    i += 2;
                } else {
                    endings.push(1);
                    i += 1;
                }
                start = i;
            }
            _ => {
                i += 1;
            }
        }
    }

    if start < bytes.len() {
        lines.push(content[start..].to_string());
        endings.push(0);
    }

    (lines, endings)
}

fn get_context_preview(lines: &[String], line_idx: usize, options: &SearchOptions) -> String {
    let before = options.before_context_lines.max(options.context_lines);
    let after = options.after_context_lines.max(options.context_lines);

    if before > 0 || after > 0 {
        let start_idx = line_idx.saturating_sub(before);
        let end_idx = (line_idx + after + 1).min(lines.len());
        lines[start_idx..end_idx].join("\n")
    } else {
        lines[line_idx].to_string()
    }
}

fn canonicalize_for_matching(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn path_matches_include(path: &Path, include_patterns: &[IncludePattern]) -> bool {
    if include_patterns.is_empty() {
        return true;
    }

    let candidate = canonicalize_for_matching(path);
    include_patterns.iter().any(|pattern| {
        if pattern.is_dir {
            candidate.starts_with(&pattern.path)
        } else {
            candidate == pattern.path
        }
    })
}

fn filter_files_by_include(
    files: Vec<PathBuf>,
    include_patterns: &[IncludePattern],
) -> Vec<PathBuf> {
    if include_patterns.is_empty() {
        return files;
    }

    files
        .into_iter()
        .filter(|path| path_matches_include(path, include_patterns))
        .collect()
}

fn build_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        if let Ok(glob) = Glob::new(pat) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

fn should_exclude_path(path: &Path, globset: &GlobSet) -> bool {
    if globset.is_match(path) {
        return true;
    }
    for component in path.components() {
        if let std::path::Component::Normal(name) = component
            && globset.is_match(name)
        {
            return true;
        }
    }
    false
}

fn collect_files(
    path: &Path,
    recursive: bool,
    exclude_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let globset = build_globset(exclude_patterns);

    if path.is_file() {
        files.push(path.to_path_buf());
    } else if recursive {
        for entry in WalkDir::new(path).into_iter().filter_entry(|e| {
            let name = e.file_name();
            !globset.is_match(e.path()) && !globset.is_match(name)
        }) {
            match entry {
                Ok(entry) => {
                    if entry.file_type().is_file() && !should_exclude_path(entry.path(), &globset) {
                        files.push(entry.path().to_path_buf());
                    }
                }
                Err(e) => {
                    tracing::debug!("Skipping path due to error: {}", e);
                    continue;
                }
            }
        }
    } else {
        match fs::read_dir(path) {
            Ok(read_dir) => {
                for entry in read_dir {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_file() && !should_exclude_path(&path, &globset) {
                                files.push(path);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Skipping directory entry due to error: {}", e);
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Cannot read directory {:?}: {}", path, e);
                return Err(e.into());
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_files(dir: &Path) -> Vec<PathBuf> {
        let files = vec![
            ("test1.txt", "hello world rust programming"),
            ("test2.rs", "fn main() { println!(\"Hello Rust\"); }"),
            ("test3.py", "print('Hello Python')"),
            ("test4.txt", "machine learning artificial intelligence"),
        ];

        let mut paths = Vec::new();
        for (name, content) in files {
            let path = dir.join(name);
            fs::write(&path, content).unwrap();
            paths.push(path);
        }
        paths
    }

    #[test]
    fn test_regex_search() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            query: "rust".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());

        let rust_matches: Vec<_> = results
            .iter()
            .filter(|r| r.preview.to_lowercase().contains("rust"))
            .collect();
        assert!(!rust_matches.is_empty());
    }

    #[test]
    fn test_regex_search_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            query: "HELLO".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            case_insensitive: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_regex_search_fixed_string() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            query: "fn main()".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            fixed_string: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_regex_search_whole_word() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("word_test.txt"),
            "rust rusty rustacean",
        )
        .unwrap();

        let options = SearchOptions {
            query: "rust".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            whole_word: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_context_lines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("context.txt");
        fs::write(&file_path, "line 1\nline 2\ntarget line\nline 4\nline 5\n").unwrap();

        let options = SearchOptions {
            query: "target".to_string(),
            path: file_path,
            recursive: false,
            context_lines: 1,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].preview.contains("line 2"));
        assert!(results[0].preview.contains("target line"));
        assert!(results[0].preview.contains("line 4"));
    }
}
