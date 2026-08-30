use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let repo = fs::canonicalize(manifest_dir.join("../..")).expect("repository root");
    let git_sha = git_stdout(&repo, &["rev-parse", "HEAD"]).to_ascii_lowercase();
    let files = source_files(&manifest_dir, &repo);
    let source_digest = digest_files(&repo, &files);

    println!("cargo:rustc-env=STS_VERIFY_BUILD_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=STS_VERIFY_BUILD_SOURCE_DIGEST={source_digest}");
    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    for name in ["HEAD", "index", "packed-refs"] {
        emit_git_rerun_path(&repo, name);
    }
    if let Some(head_ref) = git_stdout_optional(&repo, &["symbolic-ref", "-q", "HEAD"]) {
        emit_git_rerun_path(&repo, &head_ref);
    }
}

fn source_files(manifest_dir: &Path, repo: &Path) -> Vec<PathBuf> {
    let mut files = vec![
        manifest_dir.join("Cargo.toml"),
        manifest_dir.join("build.rs"),
    ];
    collect_rs_files(&manifest_dir.join("src"), &mut files);
    let core = repo.join("crates/sts_core");
    files.push(core.join("Cargo.toml"));
    collect_rs_files(&core.join("src"), &mut files);
    files.sort();
    files
}

fn collect_rs_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn digest_files(repo: &Path, files: &[PathBuf]) -> String {
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, b"sts-verify-build-source-v1");
    for file in files {
        let relative = file
            .strip_prefix(repo)
            .expect("source file under repository");
        let relative = relative.to_str().expect("UTF-8 source path");
        let contents =
            fs::read(file).unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
        hash_segment(&mut hasher, relative.as_bytes());
        hash_segment(&mut hasher, &contents);
    }
    format!("{:x}", hasher.finalize())
}

fn hash_segment(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn emit_git_rerun_path(repo: &Path, name: &str) {
    let value = git_stdout(repo, &["rev-parse", "--git-path", name]);
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        repo.join(path)
    };
    println!("cargo:rerun-if-changed={}", path.display());
}

fn git_stdout(repo: &Path, args: &[&str]) -> String {
    git_stdout_optional(repo, args).unwrap_or_else(|| panic!("git {} failed", args.join(" ")))
}

fn git_stdout_optional(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git starts");
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8(output.stdout)
            .expect("git output is UTF-8")
            .trim()
            .to_owned(),
    )
}
