use std::collections::HashMap;

pub type NodeId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl SplitDirection {
    pub fn opposite(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Window { window_id: u32 },
    Container { split: SplitDirection, children: Vec<NodeId> },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub kind: NodeKind,
    pub ratio: f32,
}

#[derive(Debug, Default)]
pub struct Tree {
    nodes: HashMap<NodeId, Node>,
    root: Option<NodeId>,
    next_id: NodeId,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&mut self) -> NodeId {
        self.next_id += 1;
        self.next_id
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn insert_window(&mut self, window_id: u32) -> NodeId {
        let id = self.alloc_id();
        let node = Node {
            id,
            parent: None,
            kind: NodeKind::Window { window_id },
            ratio: 1.0,
        };
        self.nodes.insert(id, node);

        match self.root {
            None => {
                self.root = Some(id);
            }
            Some(root_id) => {
                self.split_root(root_id, id, SplitDirection::Horizontal);
            }
        }
        id
    }

    fn split_root(&mut self, existing: NodeId, new: NodeId, split: SplitDirection) {
        let container_id = self.alloc_id();
        let parent_id = self.nodes[&existing].parent;

        let container = Node {
            id: container_id,
            parent: parent_id,
            kind: NodeKind::Container {
                split,
                children: vec![existing, new],
            },
            ratio: 1.0,
        };
        self.nodes.insert(container_id, container);

        if let Some(parent) = parent_id {
            if let NodeKind::Container { children, .. } = &mut self.nodes.get_mut(&parent).unwrap().kind {
                let pos = children.iter().position(|&c| c == existing).unwrap();
                children[pos] = container_id;
            }
        } else {
            self.root = Some(container_id);
        }

        self.nodes.get_mut(&existing).unwrap().parent = Some(container_id);
        self.nodes.get_mut(&new).unwrap().parent = Some(container_id);

        let count = 2.0f32;
        for child in [existing, new] {
            self.nodes.get_mut(&child).unwrap().ratio = 1.0 / count;
        }
    }

    pub fn remove_window(&mut self, window_id: u32) {
        let node_id = self.find_window(window_id);
        if let Some(nid) = node_id {
            self.remove_node(nid);
        }
    }

    pub fn find_window(&self, window_id: u32) -> Option<NodeId> {
        self.nodes.values().find_map(|n| {
            if let NodeKind::Window { window_id: wid } = n.kind {
                if wid == window_id { Some(n.id) } else { None }
            } else {
                None
            }
        })
    }

    fn remove_node(&mut self, id: NodeId) {
        let parent_id = self.nodes[&id].parent;
        self.nodes.remove(&id);

        if let Some(pid) = parent_id {
            let siblings = if let NodeKind::Container { children, .. } = &mut self.nodes.get_mut(&pid).unwrap().kind {
                children.retain(|&c| c != id);
                children.clone()
            } else {
                vec![]
            };

            match siblings.len() {
                0 => {
                    self.remove_node(pid);
                }
                1 => {
                    let only = siblings[0];
                    let grandparent = self.nodes[&pid].parent;
                    self.nodes.get_mut(&only).unwrap().parent = grandparent;
                    match grandparent {
                        None => self.root = Some(only),
                        Some(gp) => {
                            if let NodeKind::Container { children, .. } = &mut self.nodes.get_mut(&gp).unwrap().kind {
                                let pos = children.iter().position(|&c| c == pid).unwrap();
                                children[pos] = only;
                            }
                        }
                    }
                    self.nodes.remove(&pid);
                    self.nodes.get_mut(&only).unwrap().ratio = 1.0;
                }
                _ => {
                    let ratio = 1.0 / siblings.len() as f32;
                    for sib in siblings {
                        self.nodes.get_mut(&sib).unwrap().ratio = ratio;
                    }
                }
            }
        } else {
            self.root = None;
        }
    }

    pub fn window_ids(&self) -> Vec<u32> {
        self.nodes.values().filter_map(|n| {
            if let NodeKind::Window { window_id } = n.kind {
                Some(window_id)
            } else {
                None
            }
        }).collect()
    }

    pub fn neighbor(&self, from: NodeId, dir: Direction) -> Option<NodeId> {
        let parent_id = self.nodes.get(&from)?.parent?;
        let parent = self.nodes.get(&parent_id)?;
        if let NodeKind::Container { split, children } = &parent.kind {
            let matches = matches!(
                (split, dir),
                (SplitDirection::Horizontal, Direction::Left | Direction::Right)
                | (SplitDirection::Vertical, Direction::Up | Direction::Down)
            );
            if matches {
                let pos = children.iter().position(|&c| c == from)?;
                let next = match dir {
                    Direction::Left | Direction::Up => pos.checked_sub(1),
                    Direction::Right | Direction::Down => {
                        let n = pos + 1;
                        if n < children.len() { Some(n) } else { None }
                    }
                };
                if let Some(idx) = next {
                    return Some(self.first_leaf(children[idx]));
                }
            }
        }
        self.neighbor(parent_id, dir)
    }

    fn first_leaf(&self, id: NodeId) -> NodeId {
        match &self.nodes[&id].kind {
            NodeKind::Window { .. } => id,
            NodeKind::Container { children, .. } => self.first_leaf(children[0]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_single_window_becomes_root() {
        let mut tree = Tree::new();
        let id = tree.insert_window(1);
        assert_eq!(tree.root(), Some(id));
        assert!(matches!(tree.get(id).unwrap().kind, NodeKind::Window { window_id: 1 }));
    }

    #[test]
    fn insert_two_windows_creates_container() {
        let mut tree = Tree::new();
        tree.insert_window(1);
        tree.insert_window(2);
        let root = tree.root().unwrap();
        assert!(matches!(tree.get(root).unwrap().kind, NodeKind::Container { .. }));
    }

    #[test]
    fn remove_one_of_two_leaves_single_root() {
        let mut tree = Tree::new();
        tree.insert_window(1);
        tree.insert_window(2);
        tree.remove_window(1);
        let root = tree.root().unwrap();
        assert!(matches!(tree.get(root).unwrap().kind, NodeKind::Window { window_id: 2 }));
    }

    #[test]
    fn remove_last_window_clears_tree() {
        let mut tree = Tree::new();
        tree.insert_window(42);
        tree.remove_window(42);
        assert!(tree.root().is_none());
    }

    #[test]
    fn find_window_works() {
        let mut tree = Tree::new();
        tree.insert_window(10);
        tree.insert_window(20);
        assert!(tree.find_window(10).is_some());
        assert!(tree.find_window(99).is_none());
    }

    #[test]
    fn three_windows_neighbor_navigation() {
        let mut tree = Tree::new();
        tree.insert_window(1);
        tree.insert_window(2);
        tree.insert_window(3);

        let n1 = tree.find_window(1).unwrap();
        let _n2 = tree.find_window(2).unwrap();

        // n1 should have a right neighbor (n2 or deeper)
        let right_of_1 = tree.neighbor(n1, Direction::Right);
        assert!(right_of_1.is_some());
    }

    #[test]
    fn direction_debug_contains_name() {
        let d = Direction::Left;
        assert!(format!("{d:?}").contains("Left"));
    }
}
