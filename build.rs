//! Get the version string using `git describe --dirty` or, if it fails, using the
//! `CARGO_PKG_VERSION`.
//!
//! The `GIT` environment variable can be used to set an alternative path to the git executable.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable not set");
    let dest_path = Path::new(&out_dir).join("version.rs");
    let mut f = File::create(&dest_path).expect("Failed to create version.rs file");

    let git = env::var("GIT").unwrap_or_else(|_| {
        // On macOS, git might be installed via Homebrew or Xcode Command Line Tools
        if cfg!(target_os = "macos") {
            // Try common macOS git locations
            if Path::new("/usr/bin/git").exists() {
                "/usr/bin/git".into()
            } else if Path::new("/opt/homebrew/bin/git").exists() {
                "/opt/homebrew/bin/git".into()
            } else {
                "git".into() // Fall back to PATH
            }
        } else {
            "git".into()
        }
    });

    let description = Command::new(git).arg("describe").arg("--dirty").output();

    let cargo_version = env!("CARGO_PKG_VERSION").to_owned();

    let mut version = match description {
        Ok(output) => {
            if output.status.success() {
                match String::from_utf8(output.stdout) {
                    Ok(s) => format!("git-{}", s),
                    Err(_) => {
                        eprintln!(
                            "Warning: git describe output is not valid UTF-8, using cargo version"
                        );
                        cargo_version
                    }
                }
            } else {
                cargo_version
            }
        }
        _ => cargo_version,
    };

    // Make sure version is on a single line
    if let Some(l) = version.find('\n') {
        version.truncate(l);
    }

    writeln!(f, "#[allow(dead_code)]").expect("Failed to write to version.rs");
    writeln!(f, "pub const VERSION: &str = \"{}\";", version)
        .expect("Failed to write VERSION to version.rs");
    writeln!(f, "#[allow(dead_code)]").expect("Failed to write to version.rs");
    writeln!(f, "pub const VERSION_CSTR: &str = \"{}\\0\";", version)
        .expect("Failed to write VERSION_CSTR to version.rs");
}
