use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub node: PathBuf,
    pub tracker: PathBuf,
}

pub fn installed_runtime_paths(prefix: &Path) -> RuntimePaths {
    RuntimePaths {
        node: prefix.join("node"),
        tracker: prefix.join("tokentracker").join("bin").join("tracker.js"),
    }
}

pub fn development_runtime_paths(project_dir: &Path) -> RuntimePaths {
    RuntimePaths {
        node: project_dir.join("EmbeddedServer").join("node"),
        tracker: project_dir
            .join("EmbeddedServer")
            .join("tokentracker")
            .join("bin")
            .join("tracker.js"),
    }
}

pub fn prefix_runtime_paths_from_executable(executable: &Path) -> Option<RuntimePaths> {
    let bin_dir = executable.parent()?;
    let prefix = bin_dir.parent()?;
    Some(installed_runtime_paths(
        &prefix.join("lib").join("tokentracker-linux"),
    ))
}

fn runtime_paths_exist(paths: &RuntimePaths) -> bool {
    paths.node.exists() && paths.tracker.exists()
}

pub fn resolve_runtime_paths() -> Result<RuntimePaths, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_dir = manifest_dir
        .parent()
        .ok_or_else(|| "failed to resolve TokenTrackerLinux directory".to_string())?;
    let development = development_runtime_paths(project_dir);
    let installed = installed_runtime_paths(Path::new("/usr/lib/tokentracker-linux"));

    let executable = env::current_exe().ok();
    let running_from_repo = executable
        .as_ref()
        .is_some_and(|exe| exe.starts_with(project_dir));

    if running_from_repo && runtime_paths_exist(&development) {
        return Ok(development);
    }

    if let Some(prefix_paths) = executable
        .as_deref()
        .and_then(prefix_runtime_paths_from_executable)
    {
        if runtime_paths_exist(&prefix_paths) {
            return Ok(prefix_paths);
        }
    }

    if runtime_paths_exist(&installed) {
        return Ok(installed);
    }

    Err(format!(
        "TokenTracker runtime not found. Checked {} and {}",
        development.node.display(),
        installed.node.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_runtime_paths_use_usr_lib_layout() {
        let paths = installed_runtime_paths(Path::new("/usr/lib/tokentracker-linux"));
        assert_eq!(
            paths.node,
            PathBuf::from("/usr/lib/tokentracker-linux/node")
        );
        assert_eq!(
            paths.tracker,
            PathBuf::from("/usr/lib/tokentracker-linux/tokentracker/bin/tracker.js")
        );
    }

    #[test]
    fn development_runtime_paths_use_embedded_server_layout() {
        let paths = development_runtime_paths(Path::new("/repo/TokenTrackerLinux"));
        assert_eq!(
            paths.node,
            PathBuf::from("/repo/TokenTrackerLinux/EmbeddedServer/node")
        );
        assert_eq!(
            paths.tracker,
            PathBuf::from("/repo/TokenTrackerLinux/EmbeddedServer/tokentracker/bin/tracker.js")
        );
    }

    #[test]
    fn local_install_runtime_paths_follow_the_executable_prefix() {
        let paths = prefix_runtime_paths_from_executable(Path::new(
            "/home/user/.local/bin/tokentracker-linux",
        ))
        .expect("local executable should have a prefix");

        assert_eq!(
            paths,
            RuntimePaths {
                node: PathBuf::from("/home/user/.local/lib/tokentracker-linux/node"),
                tracker: PathBuf::from(
                    "/home/user/.local/lib/tokentracker-linux/tokentracker/bin/tracker.js"
                ),
            }
        );
    }
}
