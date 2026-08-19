use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const DEBOUNCE: Duration = Duration::from_millis(150);

pub struct SceneWatcher {
    _watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
    directory: PathBuf,
    file_name: OsString,
    changed_at: Option<Instant>,
}

impl SceneWatcher {
    pub fn new(scene_path: &Path) -> anyhow::Result<Self> {
        let file_name = scene_path
            .file_name()
            .ok_or_else(|| anyhow!("{} does not name a file", scene_path.display()))?
            .to_os_string();

        let parent = scene_path.parent().unwrap_or(Path::new(""));
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };

        let directory = std::fs::canonicalize(parent)
            .with_context(|| format!("cannot watch the directory {}", parent.display()))?;

        let (sender, events) = channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })?;

        watcher.watch(&directory, RecursiveMode::NonRecursive)?;

        Ok(Self {
            _watcher: watcher,
            events,
            directory,
            file_name,
            changed_at: None,
        })
    }

    pub fn poll(&mut self) -> bool {
        loop {
            match self.events.try_recv() {
                Ok(Ok(event)) => {
                    if self.concerns_the_scene(&event) {
                        self.changed_at = Some(Instant::now());
                    }
                }
                Ok(Err(e)) => log::warn!("scene watcher error: {e}"),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        match self.changed_at {
            Some(at) if at.elapsed() >= DEBOUNCE => {
                self.changed_at = None;
                true
            }
            _ => false,
        }
    }

    fn concerns_the_scene(&self, event: &Event) -> bool {
        if !is_content_change(event.kind) {
            return false;
        }

        event.paths.iter().any(|path| self.is_the_scene(path))
    }

    fn is_the_scene(&self, path: &Path) -> bool {
        if path.file_name() != Some(self.file_name.as_os_str()) {
            return false;
        }

        match path.parent() {
            Some(parent) => resolved(parent) == self.directory,
            None => false,
        }
    }
}

fn resolved(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn is_content_change(kind: EventKind) -> bool {
    match kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Modify(notify::event::ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        EventKind::Any | EventKind::Other => true,
        EventKind::Access(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind};

    #[test]
    fn a_write_is_a_content_change() {
        assert!(is_content_change(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
    }

    #[test]
    fn an_atomic_save_is_seen_through_its_remove_and_create() {
        assert!(
            is_content_change(EventKind::Remove(RemoveKind::File)),
            "an editor's atomic save removes the original inode"
        );
        assert!(
            is_content_change(EventKind::Create(CreateKind::File)),
            "and then creates the replacement, which is the event that matters"
        );
    }

    #[test]
    fn a_rename_into_place_is_a_content_change() {
        assert!(is_content_change(EventKind::Modify(ModifyKind::Name(
            notify::event::RenameMode::To
        ))));
    }

    #[test]
    fn a_permission_change_is_not_a_save() {
        assert!(
            !is_content_change(EventKind::Modify(ModifyKind::Metadata(
                MetadataKind::Permissions
            ))),
            "a chmod leaves the contents identical, so reloading would be pure waste"
        );
    }

    #[test]
    fn merely_reading_the_file_does_not_trigger_a_reload() {
        assert!(!is_content_change(EventKind::Access(AccessKind::Read)));
    }

    fn watcher_on(name: &str) -> (PathBuf, PathBuf, SceneWatcher) {
        let dir = std::env::temp_dir().join(format!("rust-renderer-watch-{name}"));
        std::fs::create_dir_all(&dir).expect("create the temp dir");
        let path = dir.join("scene.toml");
        std::fs::write(&path, "").expect("write the scene file");
        let watcher = SceneWatcher::new(&path).expect("the directory exists, so the watch works");
        (dir, path, watcher)
    }

    #[test]
    fn the_watched_directory_has_its_symlinks_resolved() {
        let (dir, path, watcher) = watcher_on("canonical");

        assert_eq!(
            watcher.directory,
            std::fs::canonicalize(&dir).expect("the temp dir exists"),
            "notify resolves symlinks in the paths it reports, so the watched directory must be \
             stored resolved or nothing will ever compare equal to it"
        );
        assert_eq!(watcher.file_name, OsString::from("scene.toml"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_event_about_the_scene_file_is_recognised_through_an_unresolved_path() {
        let (dir, path, watcher) = watcher_on("unresolved");

        let mut event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)));
        event.paths.push(dir.join("scene.toml"));

        assert!(
            watcher.concerns_the_scene(&event),
            "a path naming the same file through a symlinked parent must still match"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_event_about_a_neighbouring_file_is_ignored() {
        let (dir, path, watcher) = watcher_on("neighbour-unit");

        let mut event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)));
        event.paths.push(dir.join("notes.txt"));

        assert!(
            !watcher.concerns_the_scene(&event),
            "the watch covers the whole directory, so unrelated files must be filtered out"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_same_named_file_in_another_directory_is_ignored() {
        let (_dir, path, watcher) = watcher_on("same-name");

        let mut event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)));
        event.paths.push(PathBuf::from("/tmp/scene.toml"));

        assert!(
            !watcher.concerns_the_scene(&event),
            "matching on the file name alone would reload on an unrelated scene.toml"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_access_event_about_the_scene_file_is_still_ignored() {
        let (dir, path, watcher) = watcher_on("access");

        let mut event = Event::new(EventKind::Access(AccessKind::Read));
        event.paths.push(dir.join("scene.toml"));

        assert!(
            !watcher.concerns_the_scene(&event),
            "the right file but the wrong kind of event is not a save"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn watching_a_scene_file_whose_directory_is_missing_fails_loudly() {
        assert!(
            SceneWatcher::new(Path::new("/tmp/rust-renderer-no-such-dir-xyz/scene.toml")).is_err(),
            "a watch that cannot be registered must not look like a working one"
        );
    }

    #[test]
    fn a_path_that_does_not_name_a_file_is_rejected() {
        assert!(
            SceneWatcher::new(Path::new("/")).is_err(),
            "a directory is not a scene file"
        );
    }
}
