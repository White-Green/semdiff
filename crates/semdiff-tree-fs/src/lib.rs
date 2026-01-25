use memmap2::Mmap;
use mime::Mime;
use semdiff_core::{LeafTraverse, NodeTraverse, TraversalNode};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct FileMeta {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug)]
pub struct FileLeaf {
    pub name: String,
    pub abs_path: PathBuf,
    pub kind: Mime,
    pub meta: FileMeta,
    _handle: File,
    pub content: Mmap,
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
            } else {
                let handle = File::open(entry.path()).map_err(FsTreeError::Open)?;
                let content = unsafe { Mmap::map(&handle) }.map_err(FsTreeError::Open)?;
                let metadata = entry.metadata().map_err(FsTreeError::Metadata)?;
                let kind = detect_file_kind(&abs_path, &content);
                let leaf = FileLeaf {
                    name,
                    abs_path,
                    kind,
                    meta: FileMeta {
                        size: metadata.len(),
                        modified: metadata.modified().ok(),
                    },
                    _handle: handle,
                    content,
                };
                Ok(TraversalNode::Leaf(leaf))
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
