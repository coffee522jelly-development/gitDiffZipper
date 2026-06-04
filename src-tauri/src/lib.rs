use std::process::Command;
use std::fs::File;
use std::io::{Write, Read};
use std::path::Path;
use serde::{Serialize, Deserialize};
use zip::write::SimpleFileOptions;
use chrono::Local;

#[derive(Serialize, Deserialize)]
pub struct GitVersion {
    valid: bool,
    version: String,
}

#[derive(Serialize, Deserialize)]
pub struct CommitInfo {
    hash: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChangedFiles {
    files: Vec<String>,
}

// Helper to clean paths (remove quotes)
fn clean_path(path: &str) -> String {
    path.trim().trim_matches('"').trim_matches('\'').to_string()
}

// Internal logic functions
fn _validate_repo(git_path: &str, repo_path: &str) -> Result<bool, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);

    if !Path::new(&repo_path).exists() {
        return Err(format!("パスが存在しません: {}", repo_path));
    }

    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(|e| format!("Git実行エラー ({}): {}", git_path, e))?;

    Ok(output.status.success())
}

fn _validate_git(git_path: &str) -> Result<GitVersion, String> {
    let git_path = clean_path(git_path);
    let output = Command::new(&git_path)
        .arg("--version")
        .output()
        .map_err(|e| format!("Git実行エラー ({}): {}", git_path, e))?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(GitVersion {
            valid: true,
            version,
        })
    } else {
        Ok(GitVersion {
            valid: false,
            version: "".to_string(),
        })
    }
}

fn _get_commit_info(git_path: &str, repo_path: &str, offset: u32) -> Result<CommitInfo, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);
    let rev = format!("HEAD~{}", offset);
    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["log", "--format=%H%n%s", "-1", &rev])
        .output()
        .map_err(|e| format!("git log 実行エラー: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        if err.contains("ambiguous argument") {
            return Err("指定されたコミットが存在しません".to_string());
        }
        return Err(format!("Gitエラー: {}", err));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let hash = lines.next().unwrap_or("").to_string();
    let message = lines.next().unwrap_or("").to_string();

    Ok(CommitInfo { hash, message })
}

fn _get_changed_files(git_path: &str, repo_path: &str, offset: u32) -> Result<ChangedFiles, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);
    let rev = format!("HEAD~{}", offset);
    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["show", "--name-only", "--pretty=", &rev])
        .output()
        .map_err(|e| format!("git show 実行エラー: {}", e))?;

    if !output.status.success() {
        return Err(format!("Gitエラー: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = stdout.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();

    Ok(ChangedFiles { files })
}

// Tauri commands
#[tauri::command]
fn validate_repo(git_path: String, repo_path: String) -> Result<bool, String> {
    _validate_repo(&git_path, &repo_path)
}

#[tauri::command]
fn fetch_remote(git_path: String, repo_path: String) -> Result<(), String> {
    let git_path = clean_path(&git_path);
    let repo_path = clean_path(&repo_path);
    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .arg("fetch")
        .output()
        .map_err(|e| format!("git fetch 実行エラー: {}", e))?;

    if !output.status.success() {
        return Err(format!("Gitエラー: {}", String::from_utf8_lossy(&output.stderr)));
    }

    Ok(())
}

#[tauri::command]
fn validate_git(git_path: String) -> Result<GitVersion, String> {
    _validate_git(&git_path)
}

#[tauri::command]
fn get_commit_info(git_path: String, repo_path: String, offset: u32) -> Result<CommitInfo, String> {
    _get_commit_info(&git_path, &repo_path, offset)
}

#[tauri::command]
fn get_changed_files(git_path: String, repo_path: String, offset: u32) -> Result<ChangedFiles, String> {
    _get_changed_files(&git_path, &repo_path, offset)
}

#[tauri::command]
fn create_zip(
    git_path: String,
    repo_path: String,
    offset: u32,
    output_path: String,
    exclude_patterns: Vec<String>,
) -> Result<(), String> {
    let git_path = clean_path(&git_path);
    let repo_path = clean_path(&repo_path);
    let commit_info = _get_commit_info(&git_path, &repo_path, offset)?;
    let changed_files = _get_changed_files(&git_path, &repo_path, offset)?;

    let path = Path::new(&output_path);
    let file = File::create(path).map_err(|e| format!("Failed to create ZIP file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let lower_excludes: Vec<String> = exclude_patterns.iter().map(|s| s.to_lowercase()).collect();

    let mut included_files = Vec::new();
    for file_path_str in &changed_files.files {
        let lower_path = file_path_str.to_lowercase();
        let should_exclude = lower_excludes.iter().any(|p| {
            let p_trim = p.trim();
            !p_trim.is_empty() && lower_path.contains(p_trim)
        });

        if should_exclude {
            continue;
        }

        let full_path = Path::new(&repo_path).join(file_path_str);
        if full_path.is_file() {
            included_files.push(file_path_str.clone());
            let mut f = File::open(&full_path)
                .map_err(|e| format!("Failed to open file {}: {}", file_path_str, e))?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)
                .map_err(|e| format!("Failed to read file {}: {}", file_path_str, e))?;

            zip.start_file(file_path_str, options)
                .map_err(|e| format!("Failed to add file to ZIP {}: {}", file_path_str, e))?;
            zip.write_all(&buffer)
                .map_err(|e| format!("Failed to write file to ZIP {}: {}", file_path_str, e))?;
        }
    }

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut readme_content = String::new();
    readme_content.push_str("========================================\n");
    readme_content.push_str("Git差分ZIP\n");
    readme_content.push_str("========================================\n\n");
    readme_content.push_str(&format!("Commit : {}\n", commit_info.hash));
    readme_content.push_str(&format!("Message: {}\n", commit_info.message));
    readme_content.push_str(&format!("Date   : {}\n\n", now));
    readme_content.push_str(&format!("Files  : {}\n", included_files.len()));
    if !exclude_patterns.is_empty() {
        readme_content.push_str(&format!("Exclude: {}\n", exclude_patterns.join(", ")));
    }
    readme_content.push_str("\n----------------------------------------\n");
    for file_path_str in &included_files {
        readme_content.push_str(file_path_str);
        readme_content.push_str("\n");
    }
    readme_content.push_str("----------------------------------------\n");

    zip.start_file("readme.txt", options)
        .map_err(|e| format!("Failed to add readme.txt to ZIP: {}", e))?;
    zip.write_all(readme_content.as_bytes())
        .map_err(|e| format!("Failed to write readme.txt to ZIP: {}", e))?;

    zip.finish().map_err(|e| format!("Failed to finish ZIP: {}", e))?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_log::Builder::default().build())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_fs::init())
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_store::Builder::default().build())
    .invoke_handler(tauri::generate_handler![
        validate_git,
        validate_repo,
        get_commit_info,
        get_changed_files,
        fetch_remote,
        create_zip
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
