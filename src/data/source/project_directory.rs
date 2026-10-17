use std::path::{Path, PathBuf};

const HOME_MARK: &str = "~";

pub fn repository_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| dir.join(".git").exists() || dir.join(".jj").exists())
        .map(Path::to_path_buf)
}

pub fn home_abbreviated_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => HOME_MARK.to_owned(),
        Ok(rest) => format!("{HOME_MARK}/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

pub fn project_relative_directory(path: &Path, home: &Path) -> String {
    let Some(root) = repository_root(path) else {
        return home_abbreviated_path(path, home);
    };
    let Some(name) = root.file_name() else {
        return home_abbreviated_path(path, home);
    };
    let inside = path.strip_prefix(&root).unwrap_or(Path::new(""));
    if inside.as_os_str().is_empty() {
        name.to_string_lossy().into_owned()
    } else {
        Path::new(name).join(inside).display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::{env::temp_dir, fs, process};

    use super::*;

    #[test]
    fn directories_are_labelled_by_repository_then_home() {
        let scratch = temp_dir().join(format!("vetrata-label-{}", process::id()));
        let home = scratch.join("home");
        let nested = home.join("Code/vetrata/src/data");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(home.join("Code/vetrata/.jj")).unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();

        assert_eq!(
            project_relative_directory(&nested, &home),
            "vetrata/src/data"
        );
        assert_eq!(
            project_relative_directory(&home.join("Code/vetrata"), &home),
            "vetrata"
        );
        assert_eq!(
            project_relative_directory(&home.join("Downloads"), &home),
            "~/Downloads"
        );
        assert_eq!(project_relative_directory(&home, &home), "~");
        assert_eq!(project_relative_directory(Path::new("/etc"), &home), "/etc");

        fs::remove_dir_all(&scratch).unwrap();
    }
}
