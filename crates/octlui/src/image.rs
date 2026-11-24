//! Image storage. Nodes refer to images by [`ImageId`]; pixel data lives in the [`ImageStore`]
//! (shared by all windows) and is either inserted directly or loaded lazily by a supplier.

use crate::id::KeyMap;
use std::sync::Arc;

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
        assert_eq!(rgba.len(), (width * height * 4) as usize, "rgba buffer has the wrong length");
        Self { width, height, rgba }
    }
}

/// Provides image data on demand for ids that have not been inserted explicitly.
///
/// This is where decoding (PNG, JPEG, network fetches...) plugs in; octlui itself stays
/// decoder-free.
pub trait ImageSupplier {
    fn load(&mut self, id: ImageId) -> Option<ImageData>;
}

struct Entry {
    data: Option<ImageData>,
    version: u64,
}

#[derive(Default)]
pub struct ImageStore {
    images: KeyMap<Entry>,
    suppliers: Vec<Box<dyn ImageSupplier>>,
    next: u64,
    version: u64,
}

impl ImageStore {
    /// Inserts an image and returns a fresh id.
    pub fn insert(&mut self, data: ImageData) -> ImageId {
        self.next += 1;
        let id = ImageId(self.next | (1 << 63));
        self.set(id, data);
        id
    }
    pub fn insert_rgba(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        self.insert(ImageData::new(width, height, rgba))
    }
    /// Inserts or replaces the image stored under `id` (GPU copies are refreshed).
    pub fn set(&mut self, id: ImageId, data: ImageData) {
        self.version += 1;
        self.images.insert(id.0, Entry { data: Some(data), version: self.version });
    }
    pub fn remove(&mut self, id: ImageId) {
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
            let mut loaded = None;
            for s in &mut self.suppliers {
                if let Some(d) = s.load(id) {
                    loaded = Some(d);
                    break;
                }
            }
            self.version += 1;
            self.images.insert(id.0, Entry { data: loaded, version: self.version });
        }
        let e = self.images.get(&id.0)?;
        e.data.as_ref().map(|d| (d, e.version))
    }
    pub fn size(&mut self, id: ImageId) -> Option<(u32, u32)> {
        self.get(id).map(|(d, _)| (d.width, d.height))
    }
}
