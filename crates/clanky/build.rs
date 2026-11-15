//! Names the build for `clanky --version`: the last version tag plus the commit, as `git describe` gives it
//! (`1.7.0` on a tagged commit, `1.7.0-3-g675a980` three commits later). A source tarball has no git history, so
//! its packaging script writes that name into `BUILD-VERSION`; without either, the Cargo version is used.
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().unwrap_or(root);
    println!("cargo:rerun-if-env-changed=CLANKY_DESCRIBE");
    println!("cargo:rerun-if-changed={}", root.join("BUILD-VERSION").display());
    let name = std::env::var("CLANKY_DESCRIBE")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| from_git(&root))
        .or_else(|| {
            std::fs::read_to_string(root.join("BUILD-VERSION")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    println!("cargo:rustc-env=CLANKY_BUILD_VERSION={}", name.trim_start_matches('v'));
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn from_git(root: &Path) -> Option<String> {
    // Only this project's own repository: a tarball unpacked inside some other checkout must not take its tags.
    let top = PathBuf::from(git(root, &["rev-parse", "--show-toplevel"])?);
    if top.canonicalize().ok()? != root {
        return None;
    }
    let git_dir = PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"])?);
    let common = PathBuf::from(git(root, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?);
    // Build again when the commit or the tags move.
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!("cargo:rerun-if-changed={}", common.join("packed-refs").display());
    println!("cargo:rerun-if-changed={}", common.join("refs/tags").display());
    if let Some(head) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        println!("cargo:rerun-if-changed={}", common.join(head).display());
    }
    git(root, &["describe", "--tags", "--match", "v[0-9]*"])
}
