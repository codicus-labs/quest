//! Opens pages in the system browser (`open`). For scripted runs without a browser (E2E), the
//! `--browser <program>` flag names a program that gets the URL as its only argument instead.

use std::ffi::OsString;
use std::process::Command;

use quest_store::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browser {
    program: OsString,
}

impl Default for Browser {
    fn default() -> Browser {
        Browser { program: "open".into() }
    }
}

impl Browser {
    pub fn with_program(program: impl Into<OsString>) -> Browser {
        Browser { program: program.into() }
    }

    /// Starts the browser on `url` without waiting for it: a stand-in program may only finish
    /// after the sign-in's loopback listener (on the calling thread) answered it.
    pub fn open(&self, url: &Url) -> Result<(), String> {
        let mut child = Command::new(&self.program)
            .arg(url.as_str())
            .spawn()
            .map_err(|error| format!("could not open the browser ({}): {error}", self.program.to_string_lossy()))?;
        // Reaps the process once it exits.
        std::thread::spawn(move || child.wait());
        Ok(())
    }
}
