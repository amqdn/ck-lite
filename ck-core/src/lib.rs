use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Span validation error: {0}")]
    SpanValidation(String),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CkError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Haskell,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Zig,
    Dart,
    Elixir,
    Markdown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            "js" => Some(Language::JavaScript),
            "ts" | "tsx" => Some(Language::TypeScript),
            "hs" | "lhs" => Some(Language::Haskell),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "c" => Some(Language::C),
            "cpp" | "cc" | "cxx" | "c++" => Some(Language::Cpp),
            "h" | "hpp" => Some(Language::Cpp),
            "cs" => Some(Language::CSharp),
            "rb" => Some(Language::Ruby),
            "php" => Some(Language::Php),
            "swift" => Some(Language::Swift),
            "kt" | "kts" => Some(Language::Kotlin),
            "zig" => Some(Language::Zig),
            "dart" => Some(Language::Dart),
            "ex" | "exs" => Some(Language::Elixir),
            "md" | "markdown" | "mdx" => Some(Language::Markdown),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Haskell => "haskell",
            Language::Go => "go",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Zig => "zig",
            Language::Dart => "dart",
            Language::Elixir => "elixir",
            Language::Markdown => "markdown",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
}

impl Span {
    pub fn new(
        byte_start: usize,
        byte_end: usize,
        line_start: usize,
        line_end: usize,
    ) -> Result<Self> {
        let span = Self {
            byte_start,
            byte_end,
            line_start,
            line_end,
        };
        span.validate()?;
        Ok(span)
    }

    pub fn new_unchecked(
        byte_start: usize,
        byte_end: usize,
        line_start: usize,
        line_end: usize,
    ) -> Self {
        Self {
            byte_start,
            byte_end,
            line_start,
            line_end,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.line_start == 0 {
            return Err(CkError::SpanValidation(
                "Line start cannot be zero (lines are 1-indexed)".to_string(),
            ));
        }

        if self.line_end == 0 {
            return Err(CkError::SpanValidation(
                "Line end cannot be zero (lines are 1-indexed)".to_string(),
            ));
        }

        if self.byte_start > self.byte_end {
            return Err(CkError::SpanValidation(format!(
                "Invalid byte range: start ({}) > end ({})",
                self.byte_start, self.byte_end
            )));
        }

        if self.line_start > self.line_end {
            return Err(CkError::SpanValidation(format!(
                "Invalid line range: start ({}) > end ({})",
                self.line_start, self.line_end
            )));
        }

        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    pub fn byte_len(&self) -> usize {
        self.byte_end.saturating_sub(self.byte_start)
    }

    pub fn line_count(&self) -> usize {
        self.line_end.saturating_sub(self.line_start) + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub last_modified: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file: PathBuf,
    pub span: Span,
    pub score: f32,
    pub preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<Language>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_epoch: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SearchResults {
    pub matches: Vec<SearchResult>,
    pub closest_below_threshold: Option<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSignals {
    pub lex_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    pub rrf_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSearchResult {
    pub file: String,
    pub span: Span,
    pub lang: Option<Language>,
    pub symbol: Option<String>,
    pub score: f32,
    pub signals: SearchSignals,
    pub preview: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonlSearchResult {
    pub path: String,
    pub span: Span,
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_epoch: Option<u64>,
}

impl JsonlSearchResult {
    pub fn from_search_result(result: &SearchResult, include_snippet: bool) -> Self {
        Self {
            path: result.file.to_string_lossy().to_string(),
            span: result.span.clone(),
            language: result.lang.as_ref().map(std::string::ToString::to_string),
            snippet: if include_snippet {
                Some(result.preview.clone())
            } else {
                None
            },
            score: if result.score >= 0.0 {
                Some(result.score)
            } else {
                None
            },
            chunk_hash: result.chunk_hash.clone(),
            index_epoch: result.index_epoch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Regex,
}

#[derive(Debug, Clone)]
pub struct IncludePattern {
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct FileCollectionOptions {
    pub respect_gitignore: bool,
    pub use_ckignore: bool,
    pub exclude_patterns: Vec<String>,
    pub show_hidden: bool,
}

impl Default for FileCollectionOptions {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            use_ckignore: true,
            exclude_patterns: Vec::new(),
            show_hidden: false,
        }
    }
}

impl From<&SearchOptions> for FileCollectionOptions {
    fn from(opts: &SearchOptions) -> Self {
        Self {
            respect_gitignore: opts.respect_gitignore,
            use_ckignore: opts.use_ckignore,
            exclude_patterns: opts.exclude_patterns.clone(),
            show_hidden: opts.hidden,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub query: String,
    pub path: PathBuf,
    pub top_k: Option<usize>,
    pub threshold: Option<f32>,
    pub case_insensitive: bool,
    pub whole_word: bool,
    pub fixed_string: bool,
    pub line_numbers: bool,
    pub context_lines: usize,
    pub before_context_lines: usize,
    pub after_context_lines: usize,
    pub recursive: bool,
    pub json_output: bool,
    pub jsonl_output: bool,
    pub no_snippet: bool,
    pub show_scores: bool,
    pub show_filenames: bool,
    pub files_with_matches: bool,
    pub files_without_matches: bool,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<IncludePattern>,
    pub respect_gitignore: bool,
    pub use_ckignore: bool,
    pub hidden: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            mode: SearchMode::Regex,
            query: String::new(),
            path: PathBuf::from("."),
            top_k: None,
            threshold: None,
            case_insensitive: false,
            whole_word: false,
            fixed_string: false,
            line_numbers: false,
            context_lines: 0,
            before_context_lines: 0,
            after_context_lines: 0,
            recursive: true,
            json_output: false,
            jsonl_output: false,
            no_snippet: false,
            show_scores: false,
            show_filenames: false,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            include_patterns: Vec::new(),
            respect_gitignore: true,
            use_ckignore: true,
            hidden: false,
        }
    }
}

pub fn get_default_exclude_patterns() -> Vec<String> {
    vec![
        ".git".to_string(),
        ".svn".to_string(),
        ".hg".to_string(),
        "target".to_string(),
        "build".to_string(),
        "dist".to_string(),
        "node_modules".to_string(),
        ".gradle".to_string(),
        ".mvn".to_string(),
        "bin".to_string(),
        "obj".to_string(),
        "venv".to_string(),
        ".venv".to_string(),
        "env".to_string(),
        ".env".to_string(),
        "__pycache__".to_string(),
        ".vscode".to_string(),
        ".idea".to_string(),
        ".eclipse".to_string(),
        "tmp".to_string(),
        "temp".to_string(),
        ".tmp".to_string(),
    ]
}

pub fn get_default_ckignore_content() -> &'static str {
    r"# .ckignore - Default patterns for ck regex search
# Syntax: same as .gitignore (glob patterns, ! for negation)

# Images
*.png
*.jpg
*.jpeg
*.gif
*.bmp
*.svg
*.ico
*.webp
*.tiff

# Video
*.mp4
*.avi
*.mov
*.mkv
*.wmv
*.flv
*.webm

# Audio
*.mp3
*.wav
*.flac
*.aac
*.ogg
*.m4a

# Binary/Compiled
*.exe
*.dll
*.so
*.dylib
*.a
*.lib
*.obj
*.o

# Archives
*.zip
*.tar
*.tar.gz
*.tgz
*.rar
*.7z
*.bz2
*.gz

# Data files
*.db
*.sqlite
*.sqlite3
*.parquet
*.arrow
"
}

pub fn read_ckignore_patterns(repo_root: &Path) -> Result<Vec<String>> {
    let ckignore_path = repo_root.join(".ckignore");

    if !ckignore_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&ckignore_path).map_err(CkError::Io)?;

    let patterns: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(std::string::ToString::to_string)
        .collect();

    Ok(patterns)
}

pub fn create_ckignore_if_missing(repo_root: &Path) -> Result<bool> {
    let ckignore_path = repo_root.join(".ckignore");

    if ckignore_path.exists() {
        return Ok(false);
    }

    std::fs::write(&ckignore_path, get_default_ckignore_content()).map_err(CkError::Io)?;

    Ok(true)
}

pub fn build_exclude_patterns(additional_excludes: &[String], use_defaults: bool) -> Vec<String> {
    let mut patterns = Vec::new();
    patterns.extend(additional_excludes.iter().cloned());
    if use_defaults {
        patterns.extend(get_default_exclude_patterns());
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_validation() {
        let span = Span::new(0, 10, 1, 2).unwrap();
        assert_eq!(span.byte_start, 0);
        assert_eq!(span.byte_end, 10);
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 2);
        assert!(span.is_valid());

        // Zero line numbers
        assert!(Span::new(0, 10, 0, 1).is_err());
        assert!(Span::new(0, 10, 1, 0).is_err());

        // Inverted byte range
        assert!(Span::new(10, 5, 1, 1).is_err());

        // Inverted line range
        assert!(Span::new(0, 10, 2, 1).is_err());
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_exclude_patterns() {
        let excludes = vec!["custom_dir".to_string()];
        let combined = build_exclude_patterns(&excludes, true);
        assert!(combined.contains(&"custom_dir".to_string()));
        assert!(combined.contains(&".git".to_string()));

        let without_defaults = build_exclude_patterns(&excludes, false);
        assert_eq!(without_defaults, vec!["custom_dir".to_string()]);
    }
}
