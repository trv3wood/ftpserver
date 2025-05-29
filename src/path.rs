use std::path::{Path, PathBuf};

use crate::mydbg;

pub struct PathHandler {
    pwd: PathBuf,
    root: PathBuf,
}

impl PathHandler {
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        let root = root.into();
        Self {
            root: root.clone(),
            pwd: root,
        }
    }

    fn set_pwd(&mut self, new_pwd: PathBuf) {
        self.pwd = new_pwd;
    }
    pub fn cd(&mut self, new_pwd: impl Into<PathBuf>) -> std::io::Result<()> {
        let client_path: PathBuf = new_pwd.into();
        let client_path = client_path
            .strip_prefix("/")
            .unwrap_or(&client_path)
            .to_path_buf();
        let server_path = self.to_server_path(&client_path)?;
        if !mydbg!(&server_path).is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path must be absolute",
            ));
        }
        if !self.is_within_root(&server_path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Path is outside of the root directory",
            ));
        }
        self.set_pwd(server_path);
        Ok(())
    }

    pub fn get_pwd(&self) -> PathBuf {
        let path = self.to_client_path(&self.pwd);
        if path.as_os_str().is_empty() {
            PathBuf::from("/")
        } else {
            path
        }
    }

    pub fn is_within_root(&self, path: &PathBuf) -> bool {
        mydbg!(path, &self.root);
        path.starts_with(&self.root)
    }
    pub fn to_client_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        path.strip_prefix(&self.root).unwrap_or(path).to_path_buf()
    }

    pub fn to_server_path(&self, path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        let path = self.non_canonicalized_path(path)?;
        mydbg!(dunce::canonicalize(path))
    }
    pub fn non_canonicalized_path(&self, path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        let path = path.as_ref().strip_prefix("/").unwrap_or(path.as_ref());
        mydbg!(path);
        let server_path = if mydbg!(path.is_relative()) {
            let path = self.pwd.join(path);
            mydbg!(path)
        } else {
            let path = path.to_path_buf();
            mydbg!(&path);
            let local_relative = path.strip_prefix("/").map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
            })?;
            mydbg!(&local_relative);
            mydbg!(self.root.join(local_relative))
        };
        Ok(server_path)
    }
}

#[cfg(test)]
#[allow(unused)]
mod tests {
    use super::*;
    #[test]
    #[cfg(target_os = "linux")]
    fn test_to_server_path() {
        to_server_path("/var/ftp", "/dir1/doc.txt", "/var/ftp/dir1/doc.txt");
        to_server_path("/var/ftp", "/dir1", "/var/ftp/dir1");
        to_server_path("/var/ftp", "dir1/doc.txt", "/var/ftp/dir1/doc.txt");
        to_server_path("/var/ftp", "dir1", "/var/ftp/dir1");
    }
    #[test]
    #[cfg(windows)]
    fn test_to_server_path() {
        to_server_path(r"C:\\ftp", "/dir1\\doc.txt", r"C:\\ftp\\dir1\\doc.txt");
        to_server_path(r"C:\\ftp", "/dir1", r"C:\\ftp\\dir1");
        to_server_path(r"C:\\ftp", "dir1\\doc.txt", r"C:\\ftp\\dir1\\doc.txt");
        to_server_path(r"C:\\ftp", "dir1", r"C:\\ftp\\dir1");
    }

    fn to_server_path(root: &str, path: &str, expected: &str) {
        let handler = PathHandler::new(root);
        let server_path = handler.to_server_path(path).unwrap();
        assert_eq!(server_path, PathBuf::from(expected));
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn test_cd() {
        cd("/var/ftp", "dir1/doc.txt", "/var/ftp/dir1/doc.txt");
        cd("/var/ftp", "dir1", "/var/ftp/dir1");
        cd("/var/ftp", "/dir1/doc.txt", "/var/ftp/dir1/doc.txt");
        cd("/var/ftp", "/dir1", "/var/ftp/dir1");
        let mut handler = PathHandler::new("/var/ftp");
        handler.cd("dir1").unwrap();
        assert_eq!(handler.pwd, Path::new("/var/ftp/dir1"));
        handler.cd("..").unwrap();
        assert_eq!(handler.pwd, Path::new("/var/ftp"));
        handler.cd("/dir1").unwrap();
        assert_eq!(handler.pwd, Path::new("/var/ftp/dir1"));
        assert!(handler.cd("../..").is_err());
    }
    #[test]
    #[cfg(windows)]
    fn test_cd() {
        let mut handler = PathHandler::new(r"C:\\ftp");
        handler.cd("dir1").unwrap();
        assert_eq!(handler.pwd, Path::new(r"C:\\ftp\\dir1"));
        handler.cd("..").unwrap();
        assert_eq!(handler.pwd, Path::new(r"C:\\ftp"));
        handler.cd("/dir1").unwrap();
        assert_eq!(handler.pwd, Path::new(r"C:\\ftp\\dir1"));
        assert!(handler.cd("../..").is_err());
        cd(r"C:\\ftp", "dir1\\doc.txt", r"C:\\ftp\\dir1\\doc.txt");
        cd(r"C:\\ftp", "dir1", r"C:\\ftp\\dir1");
        cd(r"C:\\ftp", "/dir1\\doc.txt", r"C:\\ftp\\dir1\\doc.txt");
        cd(r"C:\\ftp", "/dir1", r"C:\\ftp\\dir1");
    }
    fn cd(root: &str, path: &str, expected: &str) {
        let mut handler = PathHandler::new(root);
        handler.cd(path).unwrap();
        assert_eq!(handler.pwd, PathBuf::from(expected));
    }
}
