//! Node identity.

use std::sync::Arc;

/// Identifies a node across frames (retained state such as scroll offsets, springs and tooltip
/// timers is keyed by it) and labels it for debugging.
#[derive(Clone, Debug)]
pub struct Id {
    pub(crate) hash: u64,
    label: Arc<str>,
}

#[inline]
pub(crate) fn mix(a: u64, b: u64) -> u64 {
    // splitmix64-style finalizer over a combined value
    let mut x = a ^ b
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(a << 6)
        .wrapping_add(a >> 2);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn fnv(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl Id {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        let label = label.into();
        Self {
            hash: mix(fnv(&label), 1),
            label,
        }
    }
    /// An id derived from this one, e.g. for list items: `Id::new("list").with_index(3)`.
    pub fn with_index(&self, index: usize) -> Id {
        Id {
            hash: mix(self.hash, index as u64 + 2),
            label: format!("{}[{}]", self.label, index).into(),
        }
    }
    pub fn label(&self) -> &str {
        &self.label
    }
}
impl PartialEq for Id {
    fn eq(&self, o: &Id) -> bool {
        self.hash == o.hash
    }
}
impl Eq for Id {}
impl std::hash::Hash for Id {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.hash.hash(h)
    }
}
impl From<&str> for Id {
    fn from(s: &str) -> Id {
        Id::new(s)
    }
}
impl From<String> for Id {
    fn from(s: String) -> Id {
        Id::new(s)
    }
}
impl From<&Id> for Id {
    fn from(s: &Id) -> Id {
        s.clone()
    }
}
impl From<(&str, usize)> for Id {
    fn from((s, i): (&str, usize)) -> Id {
        Id::new(s).with_index(i)
    }
}

/// Pass-through hasher for keys that are already well-mixed `u64`s.
#[derive(Default, Clone, Copy)]
pub(crate) struct KeyHasher(u64);
impl std::hash::Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 << 8) ^ *b as u64;
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
}
pub(crate) type KeyMap<V> =
    std::collections::HashMap<u64, V, std::hash::BuildHasherDefault<KeyHasher>>;
