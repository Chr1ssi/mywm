use std::path::PathBuf;

/// Resolve the independently versioned shell at runtime.
pub fn qml(file: &str) -> PathBuf {
    std::env::var_os("MYWM_SHELL_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("project directory has a parent")
                .join("mywm-shell/quickshell")
        })
        .join(file)
}
