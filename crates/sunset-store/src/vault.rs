//! A one-way mirror. Descriptor-relative, no-follow I/O prevents symlink/path traversal writes.
//! Only files whose bytes still equal our recorded output may be replaced or removed.
use crate::Result;
use rustix::fs::{AtFlags, CWD, Mode, OFlags, mkdirat, openat, renameat, unlinkat};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::fd::OwnedFd,
    path::{Component, Path},
};
use sunset_domain::{ConceptPage, PublicExportFile};

pub fn filename(page: &ConceptPage) -> String {
    // The id suffix makes case-folding, normalization and slug collisions harmless.
    let slug: String = page
        .concept
        .slug
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(64)
        .collect();
    format!(
        "{}-{}.md",
        if slug.is_empty() { "concept" } else { &slug },
        page.concept.id
    )
}

pub fn public_file(page: &ConceptPage) -> PublicExportFile {
    let links: Vec<_> = page.outgoing.iter().chain(&page.incoming).map(|r| {
        serde_json::json!({"direction": if r.relation.from == page.concept.id {"outgoing"} else {"incoming"}, "kind": r.relation.kind, "concept": r.other.id, "title": r.other.title})
    }).collect();
    // JSON values are valid YAML; quoting prevents frontmatter injection.
    let mut md = format!(
        "---\nid: {}\ntitle: {}\naliases: {}\nrelations: {}\n---\n\n# {}\n\n",
        page.concept.id,
        serde_json::to_string(&page.concept.title).unwrap(),
        serde_json::to_string(&page.concept.aliases).unwrap(),
        serde_json::to_string(&links).unwrap(),
        page.concept.title
    );
    if let Some(a) = &page.article {
        md.push_str(&format!("{}\n\n{}\n", a.summary, a.body_md));
    }
    if !links.is_empty() {
        md.push_str("\n## Related concepts\n\n");
    }
    for r in page.outgoing.iter().chain(&page.incoming) {
        let slug: String = r
            .other
            .slug
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(64)
            .collect();
        let stem = format!(
            "{}-{}",
            if slug.is_empty() { "concept" } else { &slug },
            r.other.id
        );
        md.push_str(&format!("- [[{stem}]] ({:?})\n", r.relation.kind));
    }
    PublicExportFile {
        filename: filename(page),
        markdown: md,
    }
}

fn io(e: rustix::io::Errno) -> std::io::Error {
    e.into()
}
fn directory(path: &Path) -> std::io::Result<OwnedFd> {
    let mut fd = openat(
        CWD,
        if path.is_absolute() { "/" } else { "." },
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io)?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::ParentDir | Component::Prefix(_)) {
                return Err(std::io::Error::other("unsafe vault path"));
            }
            continue;
        };
        match mkdirat(&fd, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => (),
            Err(e) => return Err(io(e)),
        }
        fd = openat(
            &fd,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(io)?;
    }
    Ok(fd)
}
fn child(root: &OwnedFd, private: bool) -> std::io::Result<OwnedFd> {
    if private {
        match mkdirat(
            root,
            ".private_overlay",
            Mode::RUSR | Mode::WUSR | Mode::XUSR,
        ) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => (),
            Err(e) => return Err(io(e)),
        }
    }
    openat(
        root,
        if private { ".private_overlay" } else { "." },
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io)
}
fn read(fd: &OwnedFd, name: &str) -> std::io::Result<Option<String>> {
    match openat(
        fd,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(f) => {
            let mut f = File::from(f);
            if !f.metadata()?.is_file() || f.metadata()?.len() > 8 * 1024 * 1024 {
                return Err(std::io::Error::other("not a regular file"));
            }
            let mut text = String::new();
            f.read_to_string(&mut text)?;
            Ok(Some(text))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(e) => Err(io(e)),
    }
}
fn write(fd: &OwnedFd, name: &str, text: &str) -> std::io::Result<()> {
    let tmp = format!(".mirror-{}", uuid::Uuid::now_v7());
    let f = openat(
        fd,
        &tmp,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(io)?;
    let result = (|| {
        let mut file = File::from(f);
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        renameat(fd, &tmp, fd, name).map_err(io)?;
        File::from(rustix::io::dup(fd).map_err(io)?).sync_all()
    })();
    if result.is_err() {
        let _ = unlinkat(fd, &tmp, AtFlags::empty());
    }
    result
}

pub async fn sync(
    conn: &mut SqliteConnection,
    root: &Path,
    desired: &BTreeMap<String, String>,
) -> Result<Vec<String>> {
    let root = directory(root)?;
    let rows = sqlx::query("SELECT path, content, pending_content FROM vault_files")
        .fetch_all(&mut *conn)
        .await?;
    let previous: BTreeMap<String, String> = rows
        .iter()
        .map(|r| Ok((r.try_get("path")?, r.try_get("content")?)))
        .collect::<std::result::Result<_, sqlx::Error>>()?;
    let pending: BTreeMap<String, Option<String>> = rows
        .iter()
        .map(|r| Ok((r.try_get("path")?, r.try_get("pending_content")?)))
        .collect::<std::result::Result<_, sqlx::Error>>()?;
    let paths: std::collections::BTreeSet<_> = desired.keys().chain(previous.keys()).collect();
    let mut conflicts = vec![];
    for path in paths {
        let (private, name) = match path.strip_prefix(".private_overlay/") {
            Some(n) => (true, n),
            None => (false, path.as_str()),
        };
        if name.contains('/')
            || name.contains('\\')
            || !name.ends_with(".md")
            || name.starts_with('.')
        {
            conflicts.push(path.clone());
            continue;
        }
        let fd = match child(&root, private) {
            Ok(f) => f,
            Err(_) => {
                conflicts.push(path.clone());
                continue;
            }
        };
        let current = match read(&fd, name) {
            Ok(c) => c,
            Err(_) => {
                conflicts.push(path.clone());
                continue;
            }
        };
        let target = desired.get(path);
        if current.as_ref() != target {
            if current.is_some()
                && current.as_ref() != previous.get(path)
                && current.as_ref() != pending.get(path).and_then(Option::as_ref)
            {
                conflicts.push(path.clone());
                continue;
            }
            if let Some(text) = target {
                // Record intent before touching disk: restart can recognize a completed rename
                // even if the process died before acknowledging it in the manifest.
                sqlx::query("INSERT INTO vault_files(path,content,pending_content) VALUES (?,?,?) ON CONFLICT(path) DO UPDATE SET pending_content=excluded.pending_content")
                    .bind(path).bind(previous.get(path).unwrap_or(text)).bind(text).execute(&mut *conn).await?;
                write(&fd, name, text)?;
            } else if current.is_some() {
                unlinkat(&fd, name, AtFlags::empty()).map_err(io)?;
                File::from(rustix::io::dup(&fd).map_err(io)?).sync_all()?;
            }
        }
        if let Some(text) = target {
            sqlx::query("INSERT INTO vault_files(path,content) VALUES (?,?) ON CONFLICT(path) DO UPDATE SET content=excluded.content, pending_content=NULL").bind(path).bind(text).execute(&mut *conn).await?;
        } else {
            sqlx::query("DELETE FROM vault_files WHERE path=?")
                .bind(path)
                .execute(&mut *conn)
                .await?;
        }
    }
    Ok(conflicts)
}

/// A new, public-only export. Caller provides renderer output, never arbitrary frontend files.
/// No existing snapshot is reused or cleaned up, including after an interrupted export.
pub fn export_snapshot(root: &Path, files: &[PublicExportFile]) -> Result<std::path::PathBuf> {
    let mut names = std::collections::BTreeSet::new();
    for file in files {
        let name = &file.filename;
        if name.contains('/')
            || name.contains('\\')
            || name.starts_with('.')
            || !name.ends_with(".md")
            || !names.insert(name)
        {
            return Err(std::io::Error::other("unsafe or duplicate export filename").into());
        }
    }
    let parent = directory(root)?;
    let id = uuid::Uuid::now_v7().to_string();
    mkdirat(&parent, &id, Mode::RUSR | Mode::WUSR | Mode::XUSR).map_err(io)?;
    let folder = openat(
        &parent,
        &id,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io)?;
    for file in files {
        write(&folder, &file.filename, &file.markdown)?;
    }
    File::from(parent).sync_all()?;
    Ok(root.join(id))
}
