//! Session serialization and persistence to disk.

use std::fs;
use std::path::PathBuf;

use crate::error::{MuxError, MuxResult};
use crate::session::Session;

/// Serialize a session to JSON.
pub fn serialize_session(session: &Session) -> MuxResult<String> {
    serde_json::to_string_pretty(session)
        .map_err(|e| MuxError::PersistenceError(e.to_string()))
}

/// Deserialize a session from JSON.
pub fn deserialize_session(json: &str) -> MuxResult<Session> {
    serde_json::from_str(json).map_err(|e| MuxError::PersistenceError(e.to_string()))
}

/// Return the directory where sessions are stored.
pub fn session_dir() -> PathBuf {
    dirs_or_default().join("sessions")
}

/// Return the directory where sockets are stored.
pub fn socket_dir() -> PathBuf {
    dirs_or_default().join("sockets")
}

fn dirs_or_default() -> PathBuf {
    dirs_data().unwrap_or_else(|| PathBuf::from("/tmp/alacritty"))
}

fn dirs_data() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))
        })
        .map(|d| d.join("alacritty"))
}

/// Save a session to disk.
pub fn save_session(session: &Session) -> MuxResult<()> {
    let dir = session_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", session.name));
    let json = serialize_session(session)?;
    fs::write(path, json)?;
    Ok(())
}

/// Load a session from disk by name.
pub fn load_session(name: &str) -> MuxResult<Session> {
    let path = session_dir().join(format!("{name}.json"));
    let json = fs::read_to_string(&path).map_err(|e| {
        MuxError::PersistenceError(format!("failed to read {}: {e}", path.display()))
    })?;
    deserialize_session(&json)
}

/// List all saved session names.
pub fn list_sessions() -> MuxResult<Vec<String>> {
    let dir = session_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Some(stem) = path.file_stem() {
                names.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Delete a saved session by name.
pub fn delete_session(name: &str) -> MuxResult<()> {
    let path = session_dir().join(format!("{name}.json"));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, SessionId};
    use std::env;

    fn with_temp_dir<F>(f: F)
    where
        F: FnOnce(),
    {
        let dir = env::temp_dir().join(format!("alacritty_mux_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let prev = env::var("XDG_DATA_HOME").ok();
        // SAFETY: tests run single-threaded (--test-threads=1) so no data race.
        unsafe { env::set_var("XDG_DATA_HOME", &dir) };
        f();
        // SAFETY: same as above.
        unsafe {
            env::set_var("XDG_DATA_HOME", prev.as_deref().unwrap_or(""));
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip_serialize() {
        let session = Session::new(SessionId(0), "test");
        let json = serialize_session(&session).unwrap();
        let restored = deserialize_session(&json).unwrap();
        assert_eq!(restored.name, "test");
        assert_eq!(restored.windows.len(), 1);
    }

    #[test]
    fn save_and_load() {
        with_temp_dir(|| {
            let session = Session::new(SessionId(0), "mytest");
            save_session(&session).unwrap();
            let loaded = load_session("mytest").unwrap();
            assert_eq!(loaded.name, "mytest");
        });
    }

    #[test]
    fn list_sessions_empty() {
        with_temp_dir(|| {
            let names = list_sessions().unwrap();
            assert!(names.is_empty());
        });
    }

    #[test]
    fn list_sessions_finds_saved() {
        with_temp_dir(|| {
            save_session(&Session::new(SessionId(0), "alpha")).unwrap();
            save_session(&Session::new(SessionId(1), "beta")).unwrap();
            let names = list_sessions().unwrap();
            assert_eq!(names, vec!["alpha", "beta"]);
        });
    }

    #[test]
    fn delete_session_removes_file() {
        with_temp_dir(|| {
            save_session(&Session::new(SessionId(0), "todelete")).unwrap();
            delete_session("todelete").unwrap();
            assert!(load_session("todelete").is_err());
        });
    }

    #[test]
    fn load_nonexistent_errors() {
        with_temp_dir(|| {
            assert!(load_session("doesnotexist").is_err());
        });
    }
}
