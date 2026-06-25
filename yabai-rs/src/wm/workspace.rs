use crate::layout::{
    Gaps,
    tree::{Direction, NodeKind, Tree},
    tiles::TilesLayout,
    accordion::AccordionLayout,
};
use crate::wm::{Rect, Window};
use crate::ipc::LayoutKind;

pub struct Workspace {
    name: String,
    tree: Tree,
    windows: Vec<Window>,
    layout: LayoutKind,
}

impl Workspace {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tree: Tree::new(),
            windows: vec![],
            layout: LayoutKind::Tiles,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn set_layout(&mut self, kind: LayoutKind) {
        self.layout = kind;
    }

    pub fn add_window(&mut self, win: Window) {
        self.tree.insert_window(win.id);
        self.windows.push(win);
    }

    pub fn remove_window(&mut self, window_id: u32) -> Option<Window> {
        self.tree.remove_window(window_id);
        let pos = self.windows.iter().position(|w| w.id == window_id)?;
        Some(self.windows.remove(pos))
    }

    pub fn compute_layout(&self, area: Rect, gaps: &Gaps, focused: Option<u32>) -> Vec<(u32, Rect)> {
        match self.layout {
            LayoutKind::Tiles => {
                let layout = TilesLayout { tree: &self.tree };
                layout.compute(area, gaps)
            }
            LayoutKind::Accordion => {
                let window_ids: Vec<u32> = self.windows.iter().map(|w| w.id).collect();
                let layout = AccordionLayout { padding: 30 };
                layout.compute(&window_ids, area, gaps, focused)
            }
        }
    }

    pub fn swap_with_neighbor(&mut self, window_id: u32, dir: Direction) {
        let from = match self.tree.find_window(window_id) {
            Some(n) => n,
            None => return,
        };
        let to = match self.tree.neighbor(from, dir) {
            Some(n) => n,
            None => return,
        };
        // swap the window_id values in the two Window nodes
        let from_wid = match self.tree.get(from).map(|n| n.kind.clone()) {
            Some(NodeKind::Window { window_id }) => window_id,
            _ => return,
        };
        let to_wid = match self.tree.get(to).map(|n| n.kind.clone()) {
            Some(NodeKind::Window { window_id }) => window_id,
            _ => return,
        };
        if let Some(n) = self.tree.get_mut(from) {
            n.kind = NodeKind::Window { window_id: to_wid };
        }
        if let Some(n) = self.tree.get_mut(to) {
            n.kind = NodeKind::Window { window_id: from_wid };
        }
    }
}
