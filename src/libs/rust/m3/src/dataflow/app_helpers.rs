use crate::col::String;
use crate::col::Vec;
use base::col::ToString;
use bitflags::bitflags;

/// Represent a context selector
pub type Selector = u64;

/// Context communication flags
bitflags! {
    pub struct Flags: u32 {
        const S = 0b00000001;
        const G = 0b00000010;
        const R = 0b00000100;
        const W = 0b00001000;
    }
}

/// Represents an application's offloaded context
pub struct AppContext {
    target_tile: String,
    tile_hash: [u32; 8],
    sharing: bool,
    app_logic: fn() -> i32,
}

impl<'a> AppContext {
    pub fn new(
        target_tile: String,
        tile_hash: [u32; 8],
        sharing: bool,
        app_logic: fn() -> i32,
    ) -> AppContext {
        AppContext {
            target_tile,
            tile_hash,
            sharing,
            app_logic,
        }
    }

    pub fn update_tile_hash(&mut self, tile_hash: [u32; 8]) {
        self.tile_hash = tile_hash;
    }

    pub fn update_tile_sharing(&mut self, sharing: bool) {
        self.sharing = sharing;
    }

    pub fn get_target_tile(&self) -> String {
        self.target_tile.to_string()
    }

    pub fn get_target_tile_hash(&self) -> &[u32; 8] {
        &self.tile_hash
    }

    pub fn get_tile_sharing(&self) -> bool {
        self.sharing
    }

    pub fn get_app_logic(&self) -> fn() -> i32 {
        self.app_logic
    }
}

/// Represnts a computation graph of an application
#[derive(Clone)]
pub struct CompGraph {
    pub graph: Vec<(u64, u64, Flags)>,
    index: usize,
}

impl CompGraph {
    pub fn new() -> Self {
        CompGraph {
            graph: Vec::new(),
            index: 0,
        }
    }

    pub fn create_conn(&mut self, src: u64, dst: u64, kind: Flags) {
        self.graph.push((src, dst, kind));
    }

    pub fn reset_iterator_index(&mut self) {
        self.index = 0;
    }
}

impl Iterator for CompGraph {
    type Item = (u64, u64, Flags);

    fn next(&mut self) -> Option<Self::Item> {
        let edge = self.graph[self.index];

        self.index += 1;

        Some(edge)
    }
}
