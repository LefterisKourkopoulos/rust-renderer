use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::gfx::{GpuHandle, Layouts};
use crate::scene::Scene;
use crate::scene_file;

pub enum Loaded {
    Scene(Box<Scene>),
    Failed(anyhow::Error),
}

pub struct SceneLoader {
    ctx: GpuHandle,
    scene_path: PathBuf,
    in_flight: Option<Receiver<Loaded>>,
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

    pub fn is_loading(&self) -> bool {
        self.in_flight.is_some()
    }

    pub fn scene_path(&self) -> &Path {
        &self.scene_path
    }

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

    pub fn poll(&mut self) -> Option<Loaded> {
        let receiver = self.in_flight.as_ref()?;

        let result = match receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => return None,
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
