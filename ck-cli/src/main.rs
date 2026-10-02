use anyhow::Result;
use ck_core::{SearchOptions, get_default_ckignore_content};
use clap::Parser;
use console::style;
use regex::RegexBuilder;
use std::path::{Path, PathBuf};

mod path_utils;
use path_utils::{build_include_patterns, expand_glob_patterns};

#[derive(Parser)]
#[command(name = "ck")]
#[command(about = "Fast, lightweight regex grep with gitignore awareness")]
#[command(version)]
struct Cli {
    #[arg(help = "Search pattern (regex)")]
    pattern: Option<String>,

    #[arg(help = "Files or directories to search")]
    files: Vec<PathBuf>,

    #[arg(short = 'n', long = "line-number", help = "Show line numbers")]
    line_numbers: bool,

    #[arg(long = "no-filename", help = "Suppress filenames in output")]
    no_filenames: bool,

    #[arg(short = 'H', help = "Always print filenames")]
    with_filenames: bool,

    #[arg(
        short = 'l',
        long = "files-with-matches",
        help = "Print only names of files with matches"
    )]
    files_with_matches: bool,

    #[arg(
        short = 'L',
        long = "files-without-matches",
        help = "Print only names of files without matches"
    )]
    files_without_matches: bool,

    #[arg(
        short = 'c',
        long = "count",
        help = "Only print a count of matching lines"
    )]
    count: bool,

    #[arg(short = 'i', long = "ignore-case", help = "Case insensitive search")]
    ignore_case: bool,

    #[arg(short = 'w', long = "word-regexp", help = "Match whole words only")]
    word_regexp: bool,

    #[arg(
        short = 'F',
        long = "fixed-strings",
        help = "Interpret pattern as fixed string"
    )]
    fixed_strings: bool,

    #[arg(
        short = 'R',
        short_alias = 'r',
        long = "recursive",
        help = "Recursively search directories"
    )]
    recursive: bool,

    #[arg(
        short = 'C',
        long = "context",
        value_name = "NUM",
        help = "Show NUM lines of context before and after"
    )]
    context: Option<usize>,

    #[arg(
        short = 'A',
        long = "after-context",
        value_name = "NUM",
        help = "Show NUM lines after match"
    )]
    after_context: Option<usize>,

    #[arg(
        short = 'B',
        long = "before-context",
        value_name = "NUM",
        help = "Show NUM lines before match"
    )]
    before_context: Option<usize>,

    #[arg(
        long = "topk",
        alias = "limit",
        value_name = "N",
        help = "Limit results to top N matches"
    )]
    top_k: Option<usize>,

    #[arg(long = "json", help = "Output results as JSON")]
    json: bool,

    #[arg(long = "jsonl", help = "Output results as JSONL")]
    jsonl: bool,

    #[arg(long = "no-snippet", help = "Exclude code snippets from JSONL output")]
    no_snippet: bool,

    #[arg(
        long = "exclude",
        value_name = "PATTERN",
        help = "Exclude directories matching pattern (can be used multiple times)"
    )]
    exclude: Vec<String>,

    #[arg(
        long = "no-default-excludes",
        help = "Disable default directory exclusions (like .git, node_modules, etc.)"
    )]
    no_default_excludes: bool,

    #[arg(long = "no-ignore", help = "Don't respect .gitignore files")]
    no_ignore: bool,

    #[arg(long = "no-ckignore", help = "Don't respect .ckignore file")]
    no_ckignore: bool,

    #[arg(
        long = "hidden",
        help = "Include hidden (dot-prefixed) files and directories"
    )]
    hidden: bool,

    #[arg(
        long = "print-default-ckignore",
        help = "Print the default .ckignore content that ck generates and exit"
    )]
    print_default_ckignore: bool,

    #[arg(short = 'q', long = "quiet", help = "Suppress status messages")]
    quiet: bool,
}

fn highlight_regex_matches(text: &str, pattern: &str, options: &SearchOptions) -> String {
    if options.json_output || options.jsonl_output {
        return text.to_string();
    }

    let regex_pattern = if options.fixed_string {
        regex::escape(pattern)
    } else if options.whole_word {
        format!(r"\b{}\b", regex::escape(pattern))
    } else {
        pattern.to_string()
    };

    let regex_result = RegexBuilder::new(&regex_pattern)
        .case_insensitive(options.case_insensitive)
        .build();

    match regex_result {
        Ok(re) => re
            .replace_all(text, |caps: &regex::Captures| {
                style(&caps[0]).red().bold().to_string()
            })
            .to_string(),
        Err(_) => text.to_string(),
    }
}

fn find_search_root(includes: &[ck_core::IncludePattern]) -> PathBuf {
    let mut root = match includes.first() {
        Some(pattern) => {
            if pattern.is_dir {
                pattern.path.clone()
            } else {
                pattern.path.parent().unwrap_or(&pattern.path).to_path_buf()
            }
        }
        None => return PathBuf::from("."),
    };

    for pattern in includes.iter().skip(1) {
        let mut candidate = if pattern.is_dir {
            pattern.path.clone()
        } else {
            pattern.path.parent().unwrap_or(&pattern.path).to_path_buf()
        };

        if !candidate.starts_with(&root) {
            while let Some(parent) = candidate.parent() {
                if parent.starts_with(&root) {
                    candidate = parent.to_path_buf();
                    break;
                }
                candidate = parent.to_path_buf();
            }
        }

        if root.starts_with(&candidate) {
            root = candidate;
        }
    }

    if root.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        root
    }
}

fn build_options(cli: &Cli) -> SearchOptions {
    let context = cli.context.unwrap_or(0);
    let before_context = cli.before_context.unwrap_or(context);
    let after_context = cli.after_context.unwrap_or(context);

    let exclude_patterns = ck_core::build_exclude_patterns(&cli.exclude, !cli.no_default_excludes);

    SearchOptions {
        mode: ck_core::SearchMode::Regex,
        query: cli.pattern.clone().unwrap_or_default(),
        path: PathBuf::from("."),
        top_k: cli.top_k,
        threshold: None,
        case_insensitive: cli.ignore_case,
        whole_word: cli.word_regexp,
        fixed_string: cli.fixed_strings,
        line_numbers: cli.line_numbers,
        context_lines: context,
        before_context_lines: before_context,
        after_context_lines: after_context,
        recursive: cli.recursive,
        json_output: cli.json,
        jsonl_output: cli.jsonl,
        no_snippet: cli.no_snippet,
        show_scores: false,
        show_filenames: false,
        files_with_matches: cli.files_with_matches,
        files_without_matches: cli.files_without_matches,
        exclude_patterns,
        include_patterns: Vec::new(),
        respect_gitignore: !cli.no_ignore,
        use_ckignore: !cli.no_ckignore,
        hidden: cli.hidden,
    }
}

fn canonicalize_for_comparison(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    })
}

#[cfg(unix)]
fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn main() -> Result<()> {
    reset_sigpipe();
    let cli = Cli::parse();

    if cli.print_default_ckignore {
        print!("{}", get_default_ckignore_content());
        return Ok(());
    }

    if cli.files_with_matches && cli.files_without_matches {
        eprintln!("Error: Cannot use -l and -L together");
        std::process::exit(1);
    }

    let pattern = match &cli.pattern {
        Some(p) => p.clone(),
        None => {
            eprintln!("Error: No search pattern provided. Usage: ck [OPTIONS] <PATTERN> [PATH...]");
            std::process::exit(1);
        }
    };

    let temp_options = build_options(&cli);

    let expanded_targets = if cli.files.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        expand_glob_patterns(&cli.files, &temp_options.exclude_patterns)?
    };

    let include_patterns = if cli.files.is_empty() {
        Vec::new()
    } else {
        build_include_patterns(&expanded_targets)
    };

    let mut search_root = if include_patterns.is_empty() {
        PathBuf::from(".")
    } else {
        find_search_root(&include_patterns)
    };

    if expanded_targets.len() == 1 && !expanded_targets[0].exists() {
        search_root = expanded_targets[0].clone();
    }

    let include_patterns = if include_patterns.len() > 1 {
        include_patterns
            .into_iter()
            .filter(|pattern| !(pattern.is_dir && pattern.path == search_root))
            .collect()
    } else {
        include_patterns
    };

    let mut show_filenames = if include_patterns.is_empty() {
        expanded_targets.len() > 1 || expanded_targets.iter().any(|p| p.is_dir())
    } else {
        include_patterns.len() > 1 || include_patterns.iter().any(|p| p.is_dir)
    };

    if cli.no_filenames {
        show_filenames = false;
    }
    if cli.with_filenames {
        show_filenames = true;
    }

    let mut options = build_options(&cli);
    options.show_filenames = show_filenames;
    options.include_patterns = include_patterns;
    options.path = search_root;

    let results = ck_engine::regex_search(&options)?;

    if cli.count {
        if options.show_filenames {
            let mut file_counts: std::collections::BTreeMap<PathBuf, usize> =
                std::collections::BTreeMap::new();
            for r in &results {
                *file_counts.entry(r.file.clone()).or_insert(0) += 1;
            }
            for (file, count) in file_counts {
                println!("{}:{}", file.display(), count);
            }
        } else {
            println!("{}", results.len());
        }
        if results.is_empty() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.files_with_matches {
        let mut printed_files = std::collections::HashSet::new();
        for r in &results {
            if printed_files.insert(r.file.clone()) {
                println!("{}", r.file.display());
            }
        }
        if results.is_empty() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.files_without_matches {
        let matched_canon: Vec<PathBuf> = results
            .iter()
            .map(|p| canonicalize_for_comparison(&p.file))
            .collect();

        let mut had_unmatched = false;
        for target in &expanded_targets {
            let canonical_target = canonicalize_for_comparison(target);
            let target_is_dir = target.is_dir();
            let has_match = matched_canon.iter().any(|matched| {
                if target_is_dir {
                    matched.starts_with(&canonical_target)
                } else {
                    matched == &canonical_target
                }
            });

            if !has_match {
                had_unmatched = true;
                println!("{}", target.display());
            }
        }
        if !had_unmatched {
            std::process::exit(1);
        }
        return Ok(());
    }

    if options.jsonl_output {
        for result in &results {
            let jsonl_result =
                ck_core::JsonlSearchResult::from_search_result(result, !options.no_snippet);
            println!("{}", serde_json::to_string(&jsonl_result)?);
        }
    } else if options.json_output {
        let json_results: Vec<ck_core::JsonSearchResult> = results
            .iter()
            .map(|result| ck_core::JsonSearchResult {
                file: result.file.display().to_string(),
                span: result.span.clone(),
                lang: result.lang,
                symbol: result.symbol.clone(),
                score: result.score,
                signals: ck_core::SearchSignals {
                    lex_rank: None,
                    vec_rank: None,
                    rrf_score: result.score,
                },
                preview: result.preview.clone(),
                model: "none".to_string(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        for result in &results {
            let highlighted = highlight_regex_matches(&result.preview, &pattern, &options);

            if options.line_numbers && options.show_filenames {
                println!(
                    "{}:{}:{}",
                    style(result.file.display()).cyan().bold(),
                    style(result.span.line_start).yellow(),
                    highlighted
                );
            } else if options.line_numbers {
                println!("{}:{}", style(result.span.line_start).yellow(), highlighted);
            } else if options.show_filenames {
                println!(
                    "{}:{}",
                    style(result.file.display()).cyan().bold(),
                    highlighted
                );
            } else {
                println!("{highlighted}");
            }
        }
    }

    if results.is_empty() {
        if !cli.quiet && !options.json_output && !options.jsonl_output {
            eprintln!("No matches found");
        }
        std::process::exit(1);
    }

    Ok(())
}
