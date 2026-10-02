use anyhow::Result;
use ignore::{WalkBuilder, overrides::OverrideBuilder};
use std::io::Read;
use std::path::{Path, PathBuf};

fn build_overrides(
    base_path: &Path,
    exclude_patterns: &[String],
) -> Result<ignore::overrides::Override> {
    let mut builder = OverrideBuilder::new(base_path);

    for pattern in exclude_patterns {
        if pattern.starts_with('!') {
            builder.add(pattern)?;
        } else {
            builder.add(&format!("!{pattern}"))?;
        }
    }

    Ok(builder.build()?)
}

pub fn is_text_file(path: &Path) -> bool {
    const BUFFER_SIZE: usize = 8192;

    match std::fs::File::open(path) {
        Ok(mut file) => {
            let mut buffer = vec![0; BUFFER_SIZE];
            match file.read(&mut buffer) {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        return true;
                    }
                    !buffer[..bytes_read].contains(&0)
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

fn should_include_file(entry: &ignore::DirEntry) -> bool {
    let path = entry.path();
    entry.file_type().is_some_and(|ft| ft.is_file()) && is_text_file(path)
}

fn filter_and_collect_files(walker: ignore::Walk) -> Vec<PathBuf> {
    walker
        .filter_map(std::result::Result::ok)
        .filter(should_include_file)
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

pub fn collect_files(
    path: &Path,
    options: &ck_core::FileCollectionOptions,
) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if options.respect_gitignore {
        let overrides = build_overrides(path, &options.exclude_patterns)?;
        let mut walker_builder = WalkBuilder::new(path);
        walker_builder
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .hidden(!options.show_hidden);

        if options.use_ckignore {
            walker_builder.add_custom_ignore_filename(".ckignore");
        }

        walker_builder.overrides(overrides);
        let walker = walker_builder.build();

        Ok(filter_and_collect_files(walker))
    } else {
        use ck_core::get_default_exclude_patterns;
        let mut all_patterns = get_default_exclude_patterns();
        all_patterns.extend(options.exclude_patterns.iter().cloned());
        let combined_overrides = build_overrides(path, &all_patterns)?;

        let mut walker_builder = WalkBuilder::new(path);
        walker_builder
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .hidden(!options.show_hidden);

        if options.use_ckignore {
            walker_builder.add_custom_ignore_filename(".ckignore");
        }

        walker_builder.overrides(combined_overrides);
        let walker = walker_builder.build();

        Ok(filter_and_collect_files(walker))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_is_text_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let text_file = temp_path.join("test.txt");
        let mut file = File::create(&text_file).unwrap();
        file.write_all(b"Hello world\nThis is text content")
            .unwrap();
        assert!(is_text_file(&text_file));

        let binary_file = temp_path.join("test.bin");
        let mut file = File::create(&binary_file).unwrap();
        file.write_all(&[
            0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x00, 0x57, 0x6F, 0x72, 0x6C, 0x64,
        ])
        .unwrap();
        assert!(!is_text_file(&binary_file));

        let empty_file = temp_path.join("empty.txt");
        File::create(&empty_file).unwrap();
        assert!(is_text_file(&empty_file));

        let nonexistent = temp_path.join("nonexistent.txt");
        assert!(!is_text_file(&nonexistent));
    }

    #[test]
    fn test_collect_files_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("a.txt");
        fs::write(&file_path, "content").unwrap();

        let options = ck_core::FileCollectionOptions::default();
        let collected = collect_files(&file_path, &options).unwrap();
        assert_eq!(collected, vec![file_path]);
    }

    #[test]
    fn test_collect_files_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        let file_b = temp_dir.path().join("b.rs");
        fs::write(&file_a, "alpha").unwrap();
        fs::write(&file_b, "beta").unwrap();

        let options = ck_core::FileCollectionOptions::default();
        let mut collected = collect_files(temp_dir.path(), &options).unwrap();
        collected.sort();

        let mut expected = vec![file_a, file_b];
        expected.sort();
        assert_eq!(collected, expected);
    }
}
