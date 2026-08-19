//! Loading a scene on a worker thread so the previous one keeps rendering.
//!
//! Building a scene means decoding a `.glb`, uploading its textures and running the
//! equirectangular-to-cubemap pass, which takes long enough to stall the event loop visibly. All
//! of it happens off the main thread here; only the swap itself is done on the main thread, and
//! only once the new scene is fully built.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::gfx::{GpuHandle, Layouts};
use crate::scene::Scene;
use crate::scene_file;

/// The outcome of one background load.
pub enum Loaded {
    /// The new scene, ready to be swapped in.
    Scene(Box<Scene>),
    /// The load failed. The caller keeps the scene it already has.
    Failed(anyhow::Error),
}

/// A scene load running on a worker thread.
///
/// At most one load is in flight: [`SceneLoader::request`] on a busy loader records the request as
/// pending and starts it when the current one finishes. That collapses a burst of file events —
/// editors often produce several per save — into a single reload of the *newest* content, instead
/// of queuing one load per event and rendering each in turn.
pub struct SceneLoader {
    ctx: GpuHandle,
    scene_path: PathBuf,
    in_flight: Option<Receiver<Loaded>>,
    /// Set when a request arrives while a load is already running, along with the layouts to
    /// build it against, since the follow-up starts after `request` has returned.
    pending: Option<Layouts>,
}

impl SceneLoader {
    pub fn new(ctx: GpuHandle, scene_path: PathBuf) -> Self {
        Self {
            ctx,
            scene_path,
            in_flight: None,
            pending: None,
        }
    }

    /// Whether a load is currently running.
    pub fn is_loading(&self) -> bool {
        self.in_flight.is_some()
    }

    /// The scene file this loader reads.
    pub fn scene_path(&self) -> &Path {
        &self.scene_path
    }

    /// Starts a load, or notes it as pending if one is already running.
    ///
    /// `layouts` is cloned rather than held, so the caller stays the single owner and every scene
    /// this loader produces binds against the layouts the pipelines were built from.
    pub fn request(&mut self, layouts: &Layouts) {
        if self.in_flight.is_some() {
            self.pending = Some(layouts.clone());
            return;
        }
        self.start(layouts.clone());
    }

    fn start(&mut self, layouts: Layouts) {
        let (sender, receiver) = channel();
        let ctx = self.ctx.clone();
        let path = self.scene_path.clone();

        let spawned = std::thread::Builder::new()
            .name(String::from("scene-loader"))
            .spawn(move || {
                let result = match scene_file::load(&path) {
                    Ok(config) => pollster::block_on(Scene::new(&ctx, &config, &layouts))
                        .map(|scene| Loaded::Scene(Box::new(scene)))
                        .unwrap_or_else(Loaded::Failed),
                    Err(e) => Loaded::Failed(e),
                };

                // The receiver is gone only if the window closed mid-load, in which case nobody
                // needs the result.
                let _ = sender.send(result);
            });

        match spawned {
            Ok(_) => {
                self.in_flight = Some(receiver);
                self.pending = None;
            }
            Err(e) => {
                self.pending = None;
                log::error!("cannot spawn the scene loader thread: {e}");
            }
        }
    }

    /// Collects a finished load, if there is one. Never blocks.
    ///
    /// Returns `None` while a load is still running, so it is safe to call every frame.
    pub fn poll(&mut self) -> Option<Loaded> {
        let receiver = self.in_flight.as_ref()?;

        let result = match receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => return None,
            // The thread died without sending, which means it panicked.
            Err(TryRecvError::Disconnected) => Some(Loaded::Failed(anyhow::anyhow!(
                "the scene loader thread stopped without producing a scene"
            ))),
        };

        self.in_flight = None;
        if let Some(layouts) = self.pending.take() {
            self.start(layouts);
        }

        result
    }
}
