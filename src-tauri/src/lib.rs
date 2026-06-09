use std::process::Command;
use std::fs::{self, File};
use std::io::{Write, Read, ErrorKind};
use std::path::{Path, PathBuf};
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

#[derive(Serialize, Deserialize, Clone)]
pub struct CommitHistoryItem {
    index: u32,
    hash: String,
    message: String,
    date: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChangedFiles {
    files: Vec<String>,
}

// Helper to clean paths (remove quotes and extra whitespace)
fn clean_path(path: &str) -> String {
    path.trim().trim_matches('"').trim_matches('\'').trim().to_string()
}

fn logic_get_repo_root(git_path: &str, repo_path: &str) -> Result<PathBuf, String> {
    let output = Command::new(git_path)
        .current_dir(repo_path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("Git実行エラー (rev-parse): {}", e))?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(path_str))
    } else {
        Err("Gitリポジトリのルートが見つかりません。選択したフォルダがリポジトリ内にあるか確認してください。".to_string())
    }
}

// Internal logic functions
fn logic_validate_repo(git_path: &str, repo_path: &str) -> Result<bool, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);

    if repo_path.is_empty() {
        return Ok(false);
    }

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

fn logic_validate_git(git_path: &str) -> Result<GitVersion, String> {
    let git_path = clean_path(git_path);
    if git_path.is_empty() {
        return Ok(GitVersion { valid: false, version: "".to_string() });
    }
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

fn logic_get_commit_info(git_path: &str, repo_path: &str, from_offset: u32, to_offset: u32) -> Result<CommitInfo, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);

    let (old, new) = if from_offset > to_offset {
        (from_offset, to_offset)
    } else {
        (to_offset, from_offset)
    };

    let rev_new = format!("HEAD~{}", new);
    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["log", "--format=%H%n%s", "-1", &rev_new])
        .output()
        .map_err(|e| format!("git log 実行エラー: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        if err.contains("ambiguous argument") {
            return Err(format!("指定されたコミット(HEAD~{})が存在しません。リポジトリのコミット数を確認してください。", new));
        }
        return Err(format!("Gitエラー: {}", err));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let hash = lines.next().unwrap_or("").to_string();
    let message = lines.next().unwrap_or("").to_string();

    let display_hash = if old == new {
        hash
    } else {
        format!("HEAD~{} .. {}", old, hash)
    };

    let display_message = if old == new {
        message
    } else {
        format!("[範囲] {}", message)
    };

    Ok(CommitInfo { hash: display_hash, message: display_message })
}

fn logic_get_commit_history(git_path: &str, repo_path: &str, count: u32) -> Result<Vec<CommitHistoryItem>, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);

    if repo_path.is_empty() { return Ok(vec![]); }

    // Check if there are any commits first
    let check_empty = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["rev-parse", "HEAD"])
        .output();

    if let Ok(out) = check_empty {
        if !out.status.success() {
            return Ok(vec![]);
        }
    } else {
        return Ok(vec![]);
    }

    // Use unit separator (\x1f) to avoid issues with commit messages containing pipes or commas
    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["-c", "core.quotepath=false", "log", "--format=%H\x1f%s\x1f%ai", "-n", &count.to_string()])
        .output()
        .map_err(|e| format!("git log 実行エラー: {}", e))?;

    if !output.status.success() {
        return Err(format!("Gitエラー (履歴取得): {}", String::from_utf8_lossy(&output.stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let items = stdout.lines().enumerate().map(|(i, line)| {
        let parts: Vec<&str> = line.split('\x1f').collect();
        CommitHistoryItem {
            index: i as u32,
            hash: parts.get(0).unwrap_or(&"").to_string(),
            message: parts.get(1).unwrap_or(&"").to_string(),
            date: parts.get(2).unwrap_or(&"").to_string(),
        }
    }).collect();

    Ok(items)
}

fn logic_get_changed_files(git_path: &str, repo_path: &str, from_offset: u32, to_offset: u32) -> Result<ChangedFiles, String> {
    let git_path = clean_path(git_path);
    let repo_path = clean_path(repo_path);

    if repo_path.is_empty() { return Ok(ChangedFiles { files: vec![] }); }

    let (old, new) = if from_offset > to_offset {
        (from_offset, to_offset)
    } else {
        (to_offset, from_offset)
    };

    let rev_new = format!("HEAD~{}", new);
    let rev_old = format!("HEAD~{}", old + 1);

    // Check if HEAD exists
    let check_head = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["rev-parse", "--verify", "HEAD"])
        .output();

    if let Ok(out) = check_head {
        if !out.status.success() {
            return Ok(ChangedFiles { files: vec![] });
        }
    } else {
        return Ok(ChangedFiles { files: vec![] });
    }

    // Check if rev_old exists. If not, we might be at the initial commit.
    let check_old = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(["rev-parse", "--verify", &rev_old])
        .output();

    let mut args = vec!["-c", "core.quotepath=false", "diff", "--name-only"];

    let empty_tree = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"; // SHA-1 of an empty tree

    let rev_old_resolved = if let Ok(out) = check_old {
        if out.status.success() {
            rev_old
        } else {
            empty_tree.to_string()
        }
    } else {
        empty_tree.to_string()
    };

    args.push(&rev_old_resolved);
    args.push(&rev_new);

    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .args(&args)
        .output()
        .map_err(|e| format!("git diff 実行エラー: {}", e))?;

    if !output.status.success() {
        return Err(format!("Gitエラー (差分取得): {}", String::from_utf8_lossy(&output.stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = stdout.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();

    Ok(ChangedFiles { files })
}

// Tauri commands
#[tauri::command(rename_all = "camelCase")]
fn validate_repo(git_path: String, repo_path: String) -> Result<bool, String> {
    logic_validate_repo(&git_path, &repo_path)
}

#[tauri::command(rename_all = "camelCase")]
fn get_commit_history(git_path: String, repo_path: String, count: u32) -> Result<Vec<CommitHistoryItem>, String> {
    logic_get_commit_history(&git_path, &repo_path, count)
}

#[tauri::command(rename_all = "camelCase")]
fn fetch_remote(git_path: String, repo_path: String) -> Result<(), String> {
    let git_path = clean_path(&git_path);
    let repo_path = clean_path(&repo_path);
    let output = Command::new(&git_path)
        .current_dir(&repo_path)
        .arg("fetch")
        .output()
        .map_err(|e| format!("git fetch 実行エラー: {}", e))?;

    if !output.status.success() {
        return Err(format!("Gitエラー (fetch): {}", String::from_utf8_lossy(&output.stderr)));
    }

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn validate_git(git_path: String) -> Result<GitVersion, String> {
    logic_validate_git(&git_path)
}

#[tauri::command(rename_all = "camelCase")]
fn get_commit_info(git_path: String, repo_path: String, from_offset: u32, to_offset: u32) -> Result<CommitInfo, String> {
    logic_get_commit_info(&git_path, &repo_path, from_offset, to_offset)
}

#[tauri::command(rename_all = "camelCase")]
fn get_changed_files(git_path: String, repo_path: String, from_offset: u32, to_offset: u32) -> Result<ChangedFiles, String> {
    logic_get_changed_files(&git_path, &repo_path, from_offset, to_offset)
}

#[tauri::command(rename_all = "camelCase")]
fn create_zip(
    git_path: String,
    repo_path: String,
    from_offset: u32,
    to_offset: u32,
    output_path: String,
    exclude_patterns: Vec<String>,
) -> Result<(), String> {
    let git_path = clean_path(&git_path);
    let repo_path = clean_path(&repo_path);
    let output_path = clean_path(&output_path);

    if git_path.is_empty() { return Err("Git実行ファイルが指定されていません".to_string()); }
    if repo_path.is_empty() { return Err("リポジトリが指定されていません".to_string()); }
    if output_path.is_empty() { return Err("出力先が指定されていません".to_string()); }

    // Get absolute repo root to correctly resolve relative paths from git
    let root_path = logic_get_repo_root(&git_path, &repo_path)?;

    let commit_info = logic_get_commit_info(&git_path, &repo_path, from_offset, to_offset)?;
    let changed_files = logic_get_changed_files(&git_path, &repo_path, from_offset, to_offset)?;

    let path = Path::new(&output_path);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| format!("出力先フォルダの作成に失敗しました: {}", e))?;
        }
    }

    let file = File::create(path).map_err(|e| format!("ZIPファイルの作成に失敗しました ({}): {}", output_path, e))?;
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

        let full_path = root_path.join(file_path_str);
        if full_path.is_file() {
            included_files.push(file_path_str.clone());
            let mut f = File::open(&full_path)
                .map_err(|e| match e.kind() {
                    ErrorKind::PermissionDenied => format!("アクセスが拒否されました。ファイルが他で開かれている可能性があります ({}): {}", file_path_str, e),
                    _ => format!("ファイルを開けませんでした ({}): {}", file_path_str, e),
                })?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)
                .map_err(|e| format!("ファイルを読み込めませんでした ({}): {}", file_path_str, e))?;

            // Replace Windows path separators with forward slashes for ZIP compatibility
            let zip_internal_path = file_path_str.replace('\\', "/");

            zip.start_file(&zip_internal_path, options)
                .map_err(|e| format!("ZIP内ファイル作成失敗 ({}): {}", zip_internal_path, e))?;
            zip.write_all(&buffer)
                .map_err(|e| format!("ZIP内データ書込失敗 ({}): {}", zip_internal_path, e))?;
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
        .map_err(|e| format!("readme.txtの追加に失敗しました: {}", e))?;
    zip.write_all(readme_content.as_bytes())
        .map_err(|e| format!("readme.txtの書き込みに失敗しました: {}", e))?;

    zip.finish().map_err(|e| format!("ZIPの終了処理に失敗しました: {}", e))?;

    // 5. Generate manifest.txt outside ZIP
    if let Some(parent) = path.parent() {
        let manifest_path = if parent.as_os_str().is_empty() {
            PathBuf::from("manifest.txt")
        } else {
            parent.join("manifest.txt")
        };

        let mut manifest_content = String::new();
        manifest_content.push_str(&format!("Commit ID: {}\n", commit_info.hash));
        manifest_content.push_str("Included Files:\n");
        for f in &included_files {
            manifest_content.push_str(&format!("- {}\n", f));
        }

        fs::write(manifest_path, manifest_content)
            .map_err(|e| format!("manifest.txtの書き込みに失敗しました: {}", e))?;
    }

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
        get_commit_history,
        fetch_remote,
        create_zip
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
