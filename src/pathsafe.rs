//! The one place that decides whether a path supplied by someone else (a registry document, a
//! model's tool call, a `.mz` file's `fixtures` list) may be read or written.
//!
//! Rules, applied to every read and write:
//!
//! 1. The path is relative and plain: only normal components (and `.`), no `..`, no root, no
//!    drive prefix, no NUL.
//! 2. Every component that already exists is looked at with `symlink_metadata`; a symlink anywhere
//!    on the way (a directory, the file itself, a dangling link) is refused, never followed.
//! 3. The canonical form of the deepest directory must still be under the canonical root.
//! 4. Directories are created one level at a time, never with `create_dir_all`.
//! 5. A write goes to a fresh temp file (`create_new`) in the destination directory and is renamed
//!    over the destination, so an existing link is replaced rather than written through. A read
//!    opens the file, then checks that what it opened is what the walk saw (same device and inode
//!    on unix).
//!
//! Residual: with only `std` and no `unsafe`, there is no `openat`, so a directory swapped for a
//! symlink in the microseconds between the walk and the open is caught by the re-check after the
//! temp file is made (and the file removed), not prevented.
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

fn denied(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, msg)
}

/// A plain relative path: no NUL, no root or drive prefix, no `..`.
#[must_use]
pub fn is_plain_relative(rel: &str) -> bool {
    !rel.contains('\0')
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn normals(rel: &str) -> io::Result<Vec<&OsStr>> {
    if !is_plain_relative(rel) {
        return Err(denied(format!(
            "refusing a path outside the project root: {rel}"
        )));
    }
    Ok(Path::new(rel)
        .components()
        .filter_map(|c| match c {
            Component::Normal(n) => Some(n),
            _ => None,
        })
        .collect())
}

fn symlink_refusal(p: &Path) -> io::Error {
    denied(format!(
        "refusing to follow a symlink or step through a non-directory: {}",
        p.display()
    ))
}

/// Walk `dirs` under the canonical `root`, refusing links; create missing levels when asked.
fn walk_dirs(canon_root: &Path, dirs: &[&OsStr], create: bool) -> io::Result<PathBuf> {
    let mut cur = canon_root.to_path_buf();
    for d in dirs {
        cur.push(d);
        match fs::symlink_metadata(&cur) {
            Ok(md) if md.file_type().is_dir() => {}
            Ok(_) => return Err(symlink_refusal(&cur)),
            Err(e) if create && e.kind() == io::ErrorKind::NotFound => fs::create_dir(&cur)?,
            Err(e) => return Err(e),
        }
    }
    let canon = cur.canonicalize()?;
    if canon.starts_with(canon_root) {
        Ok(canon)
    } else {
        Err(denied(format!(
            "refusing a path outside the root: {}",
            cur.display()
        )))
    }
}

/// Check `rel` against `root` without touching it, and return `root.join(rel)` (normalised).
/// `None`-like failures are `Err`. A path that does not exist yet passes if what exists is safe.
pub fn resolve(root: &Path, rel: &str) -> io::Result<PathBuf> {
    let names = normals(rel)?;
    let canon_root = root.canonicalize()?;
    let mut cur = canon_root.clone();
    let mut out = root.to_path_buf();
    for (i, n) in names.iter().enumerate() {
        cur.push(n);
        out.push(n);
        match fs::symlink_metadata(&cur) {
            Ok(md) if md.file_type().is_symlink() => return Err(symlink_refusal(&cur)),
            Ok(md) if !md.file_type().is_dir() && i + 1 < names.len() => {
                return Err(symlink_refusal(&cur));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Nothing deeper exists; the rest is plain names under a checked parent.
                for rest in &names[i + 1..] {
                    out.push(rest);
                }
                break;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Read `rel` under `root`, never following a symlink.
pub fn safe_read(root: &Path, rel: &str) -> io::Result<Vec<u8>> {
    let names = normals(rel)?;
    let canon_root = root.canonicalize()?;
    let Some((name, dirs)) = names.split_last() else {
        return Err(denied(format!(
            "refusing to read a directory as a file: {rel}"
        )));
    };
    let parent = walk_dirs(&canon_root, dirs, false)?;
    let path = parent.join(name);
    let before = fs::symlink_metadata(&path)?;
    if !before.file_type().is_file() {
        return Err(symlink_refusal(&path));
    }
    let mut f = fs::File::open(&path)?;
    let after = f.metadata()?;
    if !same_file(&before, &after) {
        return Err(denied(format!(
            "the file changed while opening: {}",
            path.display()
        )));
    }
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

/// List the entries of the directory `rel` under `root` (`.` for the root itself), never
/// following a symlink to get there.
pub fn safe_list(root: &Path, rel: &str) -> io::Result<fs::ReadDir> {
    let names = normals(rel)?;
    let canon_root = root.canonicalize()?;
    let dir = walk_dirs(&canon_root, &names, false)?;
    fs::read_dir(dir)
}

#[cfg(unix)]
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

#[cfg(not(unix))]
fn same_file(_: &fs::Metadata, _: &fs::Metadata) -> bool {
    true
}

/// Write `body` to `rel` under `root`: directories created one level at a time, links refused,
/// the file landed by renaming a fresh temp file over the destination.
pub fn safe_write(root: &Path, rel: &str, body: &[u8]) -> Result<(), String> {
    write_inner(root, rel, body).map_err(|e| e.to_string())
}

fn write_inner(root: &Path, rel: &str, body: &[u8]) -> io::Result<()> {
    let names = normals(rel)?;
    let canon_root = root.canonicalize()?;
    let Some((name, dirs)) = names.split_last() else {
        return Err(denied(format!("refusing to write an empty path: {rel}")));
    };
    let parent = walk_dirs(&canon_root, dirs, true)?;
    let dest = parent.join(name);
    match fs::symlink_metadata(&dest) {
        Ok(md) if md.file_type().is_file() => {}
        Ok(_) => return Err(symlink_refusal(&dest)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let tmp = parent.join(format!(
        ".mz-tmp-{}-{}",
        std::process::id(),
        name.to_string_lossy()
    ));
    let _ = fs::remove_file(&tmp);
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(body)?;
    }
    // The directory chain may have been swapped for a link since the walk: look again.
    let still_inside = tmp
        .canonicalize()
        .ok()
        .is_some_and(|c| c.starts_with(&canon_root) && c.parent() == Some(parent.as_path()));
    if !still_inside {
        let _ = fs::remove_file(&tmp);
        return Err(denied(format!(
            "refusing a path that changed while writing: {rel}"
        )));
    }
    fs::rename(&tmp, &dest).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn plain_relative_refuses_dotdot_absolute_nul_and_prefixes() {
        for bad in ["../a", "a/../b", "/etc/x", "a\0b", "a/..", ".."] {
            assert!(!is_plain_relative(bad), "{bad}");
        }
        for ok in ["a", "./a", "a/b.txt", ".", "a/./b"] {
            assert!(is_plain_relative(ok), "{ok}");
        }
    }

    #[test]
    fn writes_and_reads_inside_a_real_tree() {
        let d = dir();
        safe_write(d.path(), "a/b/c.txt", b"hi").unwrap();
        assert_eq!(safe_read(d.path(), "a/b/c.txt").unwrap(), b"hi");
        safe_write(d.path(), "a/b/c.txt", b"again").unwrap();
        assert_eq!(safe_read(d.path(), "a/b/c.txt").unwrap(), b"again");
        let names: Vec<_> = safe_list(d.path(), "a/b").unwrap().flatten().collect();
        assert_eq!(names.len(), 1, "no temp file is left behind");
    }

    #[test]
    fn refuses_a_symlinked_directory_for_write_read_and_list() {
        let (d, out) = (dir(), dir());
        fs::write(out.path().join("secret"), "S").unwrap();
        symlink(out.path(), d.path().join("link")).unwrap();
        assert!(safe_write(d.path(), "link/x", b"x").is_err());
        assert!(!out.path().join("x").exists());
        assert!(safe_read(d.path(), "link/secret").is_err());
        assert!(safe_list(d.path(), "link").is_err());
        assert!(resolve(d.path(), "link/secret").is_err());
    }

    #[test]
    fn refuses_a_symlinked_file_and_a_dangling_link() {
        let (d, out) = (dir(), dir());
        fs::write(out.path().join("secret"), "S").unwrap();
        symlink(out.path().join("secret"), d.path().join("f")).unwrap();
        symlink(out.path().join("nope"), d.path().join("dangling")).unwrap();
        assert!(safe_read(d.path(), "f").is_err());
        assert!(safe_write(d.path(), "f", b"x").is_err());
        assert!(safe_write(d.path(), "dangling", b"x").is_err());
        assert!(!out.path().join("nope").exists());
        assert_eq!(fs::read_to_string(out.path().join("secret")).unwrap(), "S");
    }

    #[test]
    fn refuses_dotdot_absolute_and_nul_everywhere() {
        let d = dir();
        for bad in ["../x", "a/../x", "/tmp/x", "a\0b"] {
            assert!(safe_write(d.path(), bad, b"x").is_err(), "{bad}");
            assert!(safe_read(d.path(), bad).is_err(), "{bad}");
            assert!(safe_list(d.path(), bad).is_err(), "{bad}");
            assert!(resolve(d.path(), bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_file_where_a_directory_is_needed_is_refused() {
        let d = dir();
        fs::write(d.path().join("f"), "x").unwrap();
        assert!(safe_write(d.path(), "f/x", b"x").is_err());
        assert!(resolve(d.path(), "f/x").is_err());
    }

    #[test]
    fn a_link_swapped_in_after_the_walk_is_caught_before_the_rename() {
        // Simulate the swap the open-time re-check exists for: the destination directory is
        // replaced by a link to the outside between the walk and the final rename. The temp file
        // lands through the link, the re-check sees the parent moved, removes it and refuses.
        let (d, out) = (dir(), dir());
        fs::create_dir(d.path().join("sub")).unwrap();
        let canon_root = d.path().canonicalize().unwrap();
        let parent = walk_dirs(&canon_root, &[OsStr::new("sub")], true).unwrap();
        fs::remove_dir(d.path().join("sub")).unwrap();
        symlink(out.path(), d.path().join("sub")).unwrap();
        let tmp = parent.join(".mz-tmp-test");
        let landed_outside =
            fs::write(&tmp, "x").is_ok() && out.path().join(".mz-tmp-test").exists();
        assert!(
            landed_outside,
            "the simulated swap puts the temp file outside"
        );
        let still_inside = tmp
            .canonicalize()
            .ok()
            .is_some_and(|c| c.starts_with(&canon_root));
        assert!(!still_inside, "the re-check used by safe_write detects it");
    }
}
