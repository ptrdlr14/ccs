use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "ccs-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ccs"));
        command.env("HOME", &self.root);
        command
    }

    pub fn config(&self, content: &str) {
        let dir = self.root.join(".config/ccs");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), content).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
