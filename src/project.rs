use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use std::io;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectRegistryEntry {
    pub name: String,
    pub root: PathBuf,
    pub last_opened: DateTime<Utc>,
}

pub struct ProjectRegistry {
    path: PathBuf,
}

impl ProjectRegistry {
    pub fn new() -> Self {
        let mut path = home::home_dir().expect("Could not find home directory");
        path.push(".config");
        path.push("heph");
        path.push("projects.json");
        Self { path }
    }

    pub fn load(&self) -> io::Result<Vec<ProjectRegistryEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path)?;
        let entries: Vec<ProjectRegistryEntry> = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(entries)
    }

    pub fn save(&self, entries: &[ProjectRegistryEntry]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(entries)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn resolve(&self, name_or_path: &str) -> io::Result<PathBuf> {
        let path = Path::new(name_or_path);
        if path.is_dir() {
            return Ok(fs::canonicalize(path)?);
        }

        let entries = self.load()?;
        if let Some(entry) = entries.iter().find(|e| e.name == name_or_path) {
            return Ok(entry.root.clone());
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Project not found: {}", name_or_path),
        ))
    }

    pub fn find_by_name(&self, name: &str) -> io::Result<Option<ProjectRegistryEntry>> {
        let entries = self.load()?;
        Ok(entries.into_iter().find(|e| e.name == name))
    }

    pub fn list(&self) -> io::Result<Vec<ProjectRegistryEntry>> {
        self.load()
    }

    pub fn remember(&self, name: String, root: PathBuf) -> io::Result<()> {
        let mut entries = self.load()?;
        let abs_root = if root.exists() { fs::canonicalize(root)? } else { root };
        
        if let Some(entry) = entries.iter_mut().find(|e| e.root == abs_root || e.name == name) {
            entry.name = name;
            entry.root = abs_root;
            entry.last_opened = Utc::now();
        } else {
            entries.push(ProjectRegistryEntry {
                name,
                root: abs_root,
                last_opened: Utc::now(),
            });
        }
        self.save(&entries)
    }

    pub fn remove(&self, name: &str) -> io::Result<()> {
        let mut entries = self.load()?;
        let original_len = entries.len();
        entries.retain(|e| e.name != name);
        if entries.len() < original_len {
            self.save(&entries)?;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Project '{}' not found in registry", name),
            ))
        }
    }
}

pub struct Project {
    pub root: PathBuf,
    pub name: String,
}

impl Project {
    pub fn new(root: PathBuf) -> Self {
        let name = root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();
        Self { root, name }
    }
}
