use crate::{LeafTraverse, NodeTraverse, TraversalNode};
use memmap2::Mmap;
use mime::Mime;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct FileLeaf {
    pub name: String,
    pub kind: Mime,
    pub content: Arc<Mmap>,
}

impl LeafTraverse for FileLeaf {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Error)]
pub enum FsTreeError {
    #[error("failed to read directory: {0}")]
    ReadDir(#[source] io::Error),
    #[error("failed to read file metadata: {0}")]
    Metadata(#[source] io::Error),
    #[error("failed to open file: {0}")]
    Open(io::Error),
    #[error("unsupported file type: {0:?}")]
    UnsupportedFileType(PathBuf),
}

#[derive(Clone, Debug)]
pub struct FsNode {
    abs_path: PathBuf,
    name: String,
}

impl FsNode {
    pub fn new_root(path: PathBuf) -> FsNode {
        FsNode {
            abs_path: path,
            name: "".to_owned(),
        }
    }

    fn new(abs_path: PathBuf, name: String) -> Self {
        Self { abs_path, name }
    }
}

impl NodeTraverse for FsNode {
    type Leaf = FileLeaf;

    type TraverseError = FsTreeError;

    fn name(&self) -> &str {
        &self.name
    }

    fn children(
        &mut self,
    ) -> Result<impl Iterator<Item = Result<TraversalNode<Self, Self::Leaf>, Self::TraverseError>>, Self::TraverseError>
    {
        let entries = match std::fs::read_dir(&self.abs_path) {
            Ok(entries) => entries,
            Err(err) => return Err(FsTreeError::ReadDir(err)),
        };

        Ok(entries.map(|entry| {
            let entry = entry.map_err(FsTreeError::ReadDir)?;
            let file_type = entry.file_type().map_err(FsTreeError::Metadata)?;
            let name = entry.file_name();
            let abs_path = entry.path();
            let name = name.to_string_lossy().into_owned();
            if file_type.is_dir() {
                Ok(TraversalNode::Node(FsNode::new(abs_path, name)))
            } else if file_type.is_file() {
                let file = open_regular_file(&abs_path)?;
                let content = unsafe { Mmap::map(&file) }.map_err(FsTreeError::Open)?;
                let kind = detect_file_kind(&abs_path, &content);
                let leaf = FileLeaf {
                    name,
                    kind,
                    content: Arc::new(content),
                };
                Ok(TraversalNode::Leaf(leaf))
            } else {
                Err(FsTreeError::UnsupportedFileType(abs_path))
            }
        }))
    }
}

fn detect_file_kind(path: &Path, body: &[u8]) -> Mime {
    if let Some(kind) = infer::get(body)
        && let Ok(mime) = kind.mime_type().parse()
    {
        mime
    } else if let Some(mime) = mime_guess::from_path(path).first() {
        mime
    } else {
        mime::APPLICATION_OCTET_STREAM
    }
}

#[cfg(unix)]
fn open_regular_file(path: &Path) -> Result<File, FsTreeError> {
    use std::os::unix::fs::OpenOptionsExt;

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|err| {
            if err.raw_os_error() == Some(libc::ELOOP) {
                FsTreeError::UnsupportedFileType(path.to_path_buf())
            } else {
                FsTreeError::Open(err)
            }
        })?;
    validate_regular_file(file, path)
}

#[cfg(windows)]
fn open_regular_file(path: &Path) -> Result<File, FsTreeError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(FsTreeError::Open)?;
    validate_regular_file(file, path)
}

#[cfg(not(any(unix, windows)))]
fn open_regular_file(path: &Path) -> Result<File, FsTreeError> {
    let file = OpenOptions::new().read(true).open(path).map_err(FsTreeError::Open)?;
    validate_regular_file(file, path)
}

fn validate_regular_file(file: File, path: &Path) -> Result<File, FsTreeError> {
    let file_type = file.metadata().map_err(FsTreeError::Metadata)?.file_type();
    if file_type.is_file() {
        Ok(file)
    } else {
        Err(FsTreeError::UnsupportedFileType(path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeTraverse;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("semdiff-core-{name}-{nanos}"))
    }

    #[test]
    fn fs_node_reads_regular_file_children() {
        let root = unique_temp_path("regular-file");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("sample.txt"), "hello").unwrap();

        let mut node = FsNode::new_root(root.clone());
        let mut children = node.children().unwrap();
        let child = children.next().unwrap().unwrap();

        match child {
            TraversalNode::Leaf(leaf) => {
                assert_eq!(leaf.name, "sample.txt");
                assert_eq!(&leaf.content[..], b"hello");
            }
            TraversalNode::Node(_) => panic!("regular file was returned as a node"),
        }

        assert!(children.next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn fs_node_rejects_symlink_file_children() {
        let root = unique_temp_path("symlink-root");
        fs::create_dir(&root).unwrap();
        let outside_secret = unique_temp_path("outside-secret");
        fs::write(&outside_secret, "SEMDIFF_SECRET_OUTSIDE_ROOT_12345").unwrap();
        let link_path = root.join("innocent.txt");

        cfg_select! {
            unix => {
                std::os::unix::fs::symlink(&outside_secret, &link_path).unwrap();
            }
            windows => {
                if let Err(err) = std::os::windows::fs::symlink_file(&outside_secret, &link_path) {
                    fs::remove_dir_all(&root).unwrap();
                    fs::remove_file(&outside_secret).unwrap();
                    if err.kind() == io::ErrorKind::PermissionDenied {
                        return;
                    }
                    panic!("failed to create Windows symlink: {err}");
                }
            }
        }

        assert!(matches!(
            open_regular_file(&link_path),
            Err(FsTreeError::UnsupportedFileType(path)) if path == link_path
        ));

        let mut node = FsNode::new_root(root.clone());
        let mut children = node.children().unwrap();
        let child = children.next().unwrap();

        match child {
            Err(FsTreeError::UnsupportedFileType(path)) => assert_eq!(path, link_path),
            Ok(TraversalNode::Leaf(leaf)) => panic!(
                "symlink target contents were exposed as a leaf: {:?}",
                String::from_utf8_lossy(&leaf.content)
            ),
            Ok(TraversalNode::Node(_)) => panic!("symlink was returned as a node"),
            Err(err) => panic!("unexpected symlink error: {err}"),
        }

        assert!(children.next().is_none());
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside_secret).unwrap();
    }
}
