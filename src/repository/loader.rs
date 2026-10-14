use crate::analysis::analyse;
use crate::repository::{LineSpan, SourceFile};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;

const MAX_FILE_BYTES: usize = 1024 * 1024;
const BINARY_SNIFF_BYTES: usize = 8000;
const TAB_REPLACEMENT: &str = "    ";
const SKIPPED_DIRECTORY_NAMES: [&str; 3] = ["target", "node_modules", ".git"];
const MAX_EXPANDED_LINES: usize = 4000;
const COLLAPSED_FILE_NAMES: [&str; 5] = [
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
];

pub struct LoadedFile {
    pub path: PathBuf,
    pub source: SourceFile,
}

pub fn load_files(root: &Path) -> io::Result<Vec<LoadedFile>> {
    let paths = match list_git_files(root) {
        Some(paths) => paths,
        None => list_files_recursively(root, Path::new(""))?,
    };
    Ok(load_in_parallel(root, paths))
}

fn load_in_parallel(root: &Path, paths: Vec<PathBuf>) -> Vec<LoadedFile> {
    let thread_count = thread::available_parallelism().map_or(4, |count| count.get());
    let chunk_size = paths.len().div_ceil(thread_count).max(1);
    thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(move || load_chunk(root, chunk)))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

fn load_chunk(root: &Path, paths: &[PathBuf]) -> Vec<LoadedFile> {
    paths
        .iter()
        .filter_map(|path| load_file(root, path))
        .collect()
}

fn load_file(root: &Path, path: &Path) -> Option<LoadedFile> {
    let bytes = fs::read(root.join(path)).ok()?;
    let text = decode_text(&bytes).filter(|text| !check_is_collapsed(path, text));
    let line_spans = text.as_deref().map(measure_lines).unwrap_or_default();
    let analysis = text.as_deref().and_then(|text| analyse(path, text));
    Some(LoadedFile {
        path: path.to_path_buf(),
        source: SourceFile {
            text: text.map(Arc::from),
            line_spans,
            analysis,
        },
    })
}

fn decode_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_FILE_BYTES || check_is_binary(bytes) {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    Some(text.replace('\r', "").replace('\t', TAB_REPLACEMENT))
}

fn check_is_collapsed(path: &Path, text: &str) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    COLLAPSED_FILE_NAMES.contains(&name.as_ref()) || text.lines().count() > MAX_EXPANDED_LINES
}

fn check_is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_SNIFF_BYTES)].contains(&0)
}

fn measure_lines(text: &str) -> Vec<LineSpan> {
    text.lines().map(measure_line).collect()
}

fn measure_line(line: &str) -> LineSpan {
    let length = line.chars().count() as u32;
    let trimmed_length = line.trim_start().chars().count() as u32;
    LineSpan {
        indent: length - trimmed_length,
        length,
    }
}

fn list_git_files(root: &Path) -> Option<Vec<PathBuf>> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| PathBuf::from(String::from_utf8_lossy(entry).into_owned()))
        .collect();
    Some(paths)
}

fn list_files_recursively(root: &Path, relative: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() && !check_is_skipped_directory(&path) {
            paths.extend(list_files_recursively(root, &path)?);
        }
        if file_type.is_file() {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn check_is_skipped_directory(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    name.starts_with('.') || SKIPPED_DIRECTORY_NAMES.contains(&name.as_ref())
}
