//! Image storage. Nodes refer to images by [`ImageId`]; pixel data lives in the [`ImageStore`]
//! (shared by all windows) and is either inserted directly or loaded lazily by a supplier.

use crate::id::KeyMap;
use std::io;
use std::sync::{Arc, mpsc};
use winit::event_loop::EventLoopProxy;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

impl ImageId {
    /// Derives a stable id from a name, so supplier-backed images can be addressed by path/URL.
    pub fn from_name(name: &str) -> ImageId {
        ImageId(crate::id::mix(Id::new(name).hash, 0x1a6e))
    }
}
use crate::id::Id;

/// Decoded RGBA8 (non-premultiplied, sRGB-encoded) pixels.
#[derive(Clone, Debug)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}
impl ImageData {
    pub fn new(width: u32, height: u32, rgba: impl Into<Arc<[u8]>>) -> Self {
        let rgba = rgba.into();
        assert_eq!(
            rgba.len(),
            (width * height * 4) as usize,
            "rgba buffer has the wrong length"
        );
        Self {
            width,
            height,
            rgba,
        }
    }
}

/// Provides image data on demand for ids that have not been inserted explicitly.
///
/// Runs synchronously for raw RGBA sources. Use
/// [`crate::AppContext::register_image_supplier`] to load and decode encoded data off-thread.
pub trait ImageSupplier {
    fn load(&mut self, id: ImageId) -> Option<ImageData>;
}

struct Entry {
    data: Option<ImageData>,
    version: u64,
}

type Source = Box<dyn FnOnce() -> io::Result<Vec<u8>> + Send>;
type DecodeJob = (ImageId, u64, Source);
type DecodeResult = (ImageId, u64, Option<ImageData>);

struct Decoder {
    sources: KeyMap<Source>,
    pending: KeyMap<u64>,
    jobs: mpsc::Sender<DecodeJob>,
    results: mpsc::Receiver<DecodeResult>,
}

impl Decoder {
    fn new(wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        let (jobs, requests) = mpsc::channel::<DecodeJob>();
        let (finish, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("octlui image decoder".into())
            .spawn(move || {
                while let Ok((id, version, source)) = requests.recv() {
                    let data = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        decode(&source().ok()?)
                    }))
                    .ok()
                    .flatten();
                    if finish.send((id, version, data)).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self {
            sources: KeyMap::default(),
            pending: KeyMap::default(),
            jobs,
            results,
        })
    }
}

fn decode(bytes: &[u8]) -> Option<ImageData> {
    let mut reader = ::image::ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.into_rgba8();
    Some(ImageData::new(rgba.width(), rgba.height(), rgba.into_raw()))
}

#[derive(Default)]
pub struct ImageStore {
    images: KeyMap<Entry>,
    suppliers: Vec<Box<dyn ImageSupplier>>,
    next: u64,
    version: u64,
    decoder: Option<Decoder>,
}

impl ImageStore {
    /// Inserts an image and returns a fresh id.
    pub fn insert(&mut self, data: ImageData) -> ImageId {
        let id = self.next_id();
        self.set(id, data);
        id
    }
    fn next_id(&mut self) -> ImageId {
        self.next += 1;
        ImageId(self.next | (1 << 63))
    }

    pub(crate) fn register_encoded(
        &mut self,
        source: Source,
        proxy: EventLoopProxy,
    ) -> io::Result<ImageId> {
        let id = self.next_id();
        if self.decoder.is_none() {
            self.decoder = Some(Decoder::new(move || proxy.wake_up())?);
        }
        self.decoder.as_mut().unwrap().sources.insert(id.0, source);
        Ok(id)
    }

    pub(crate) fn resolve_decoded(&mut self) -> bool {
        let Some(decoder) = &mut self.decoder else {
            return false;
        };
        let mut changed = false;
        for (id, version, data) in decoder.results.try_iter() {
            if decoder.pending.get(&id.0) != Some(&version) {
                continue;
            }
            decoder.pending.remove(&id.0);
            changed |= data.is_some();
            self.images.insert(id.0, Entry { data, version });
        }
        changed
    }

    fn cancel_decode(&mut self, id: ImageId) {
        if let Some(decoder) = &mut self.decoder {
            decoder.sources.remove(&id.0);
            decoder.pending.remove(&id.0);
        }
    }
    pub fn insert_rgba(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        self.insert(ImageData::new(width, height, rgba))
    }
    /// Inserts or replaces the image stored under `id` (GPU copies are refreshed).
    pub fn set(&mut self, id: ImageId, data: ImageData) {
        self.cancel_decode(id);
        self.version += 1;
        self.images.insert(
            id.0,
            Entry {
                data: Some(data),
                version: self.version,
            },
        );
    }
    pub fn remove(&mut self, id: ImageId) {
        self.cancel_decode(id);
        self.images.remove(&id.0);
    }
    /// Adds a supplier consulted (in insertion order) for unknown ids.
    pub fn add_supplier(&mut self, supplier: impl ImageSupplier + 'static) {
        self.suppliers.push(Box::new(supplier));
    }
    /// Looks an image up, asking the suppliers if it is not stored yet. A failed lookup is
    /// remembered so suppliers are not asked again every frame.
    pub fn get(&mut self, id: ImageId) -> Option<(&ImageData, u64)> {
        if !self.images.contains_key(&id.0) {
            if let Some(decoder) = &mut self.decoder {
                if let Some(source) = decoder.sources.remove(&id.0) {
                    self.version += 1;
                    if decoder.jobs.send((id, self.version, source)).is_ok() {
                        decoder.pending.insert(id.0, self.version);
                    }
                }
                if decoder.pending.contains_key(&id.0) {
                    return None;
                }
            }
            let mut loaded = None;
            for s in &mut self.suppliers {
                if let Some(d) = s.load(id) {
                    loaded = Some(d);
                    break;
                }
            }
            self.version += 1;
            self.images.insert(
                id.0,
                Entry {
                    data: loaded,
                    version: self.version,
                },
            );
        }
        let e = self.images.get(&id.0)?;
        e.data.as_ref().map(|d| (d, e.version))
    }
    pub fn size(&mut self, id: ImageId) -> Option<(u32, u32)> {
        self.get(id).map(|(d, _)| (d.width, d.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    use std::time::Duration;

    #[test]
    fn decode_starts_on_first_use_and_respects_replacement() {
        let (wake, completed) = mpsc::channel();
        let mut store = ImageStore {
            decoder: Some(
                Decoder::new(move || {
                    let _ = wake.send(());
                })
                .unwrap(),
            ),
            ..Default::default()
        };
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(&[11, 22, 33, 255], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        let id = store.next_id();
        let replaced = store.next_id();
        let (send, release) = mpsc::channel();
        let decoder = store.decoder.as_mut().unwrap();
        let first = bytes.clone();
        decoder.sources.insert(id.0, Box::new(move || Ok(first)));
        decoder.sources.insert(
            replaced.0,
            Box::new(move || {
                release.recv().map_err(io::Error::other)?;
                Ok(bytes)
            }),
        );
        assert!(completed.try_recv().is_err());
        assert!(store.get(id).is_none());
        assert!(store.get(id).is_none());
        completed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(store.resolve_decoded());
        assert_eq!(&*store.get(id).unwrap().0.rgba, &[11, 22, 33, 255]);
        assert!(store.get(replaced).is_none());
        store.set(replaced, ImageData::new(1, 1, vec![55, 66, 77, 255]));
        send.send(()).unwrap();
        completed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(!store.resolve_decoded());
        assert_eq!(&*store.get(replaced).unwrap().0.rgba, &[55, 66, 77, 255]);
    }
}
