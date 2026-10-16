//! Which build of WST is running, to tell what an earlier build made.

use std::{
    fs::File,
    hash::{DefaultHasher, Hasher},
    io::Read,
    sync::OnceLock,
};

/// This build of WST: a hash of the running executable, so an interpretation
/// another build made, e.g. before a deploy, can be told apart. Only told
/// apart from other builds', not stable across Rust's versions, which build
/// other executables anyway. None if the executable can't be read, when no
/// build's interpretation can be known for this one's.
pub fn current_build() -> Option<&'static str> {
    static BUILD: OnceLock<Option<String>> = OnceLock::new();
    BUILD.get_or_init(hash_executable).as_deref()
}

fn hash_executable() -> Option<String> {
    let mut file = File::open(std::env::current_exe().ok()?).ok()?;
    let mut hasher = DefaultHasher::new();
    let mut buffer = vec![0; 1 << 20];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.write(&buffer[..read]);
    }
    Some(format!("{:016x}", hasher.finish()))
}
