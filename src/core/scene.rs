use std::fmt;

use crate::{
    output::OutputId,
    window::{WindowGeometry, WindowId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneNodeId(u64);

impl SceneNodeId {
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for SceneNodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneNodeKind {
    Output(OutputId),
    Window(WindowId),
    Surface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneNode {
    pub id: SceneNodeId,
    pub parent: Option<SceneNodeId>,
    pub kind: SceneNodeKind,
    pub geometry: WindowGeometry,
    pub children: Vec<SceneNodeId>,
}

impl SceneNode {
    #[must_use]
    pub fn new(
        id: SceneNodeId,
        parent: Option<SceneNodeId>,
        kind: SceneNodeKind,
        geometry: WindowGeometry,
    ) -> Self {
        Self {
            id,
            parent,
            kind,
            geometry,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Scene {
    nodes: Vec<SceneNode>,
    next_node_id: u64,
}

impl Scene {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_node_id: 1,
        }
    }

    pub fn add_output(&mut self, output_id: OutputId) -> SceneNodeId {
        self.add_node(
            None,
            SceneNodeKind::Output(output_id),
            WindowGeometry::default(),
        )
    }

    pub fn add_window(
        &mut self,
        output_node: SceneNodeId,
        window_id: WindowId,
        geometry: WindowGeometry,
    ) -> SceneNodeId {
        self.add_node(
            Some(output_node),
            SceneNodeKind::Window(window_id),
            geometry,
        )
    }

    pub fn add_surface(
        &mut self,
        window_node: SceneNodeId,
        geometry: WindowGeometry,
    ) -> SceneNodeId {
        self.add_node(Some(window_node), SceneNodeKind::Surface, geometry)
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn window_geometries_for_output(
        &self,
        output_width: u32,
        output_height: u32,
    ) -> Vec<WindowGeometry> {
        self.nodes
            .iter()
            .filter_map(|node| match node.kind {
                SceneNodeKind::Window(_) => Some(
                    node.geometry
                        .with_default_for_output(output_width, output_height),
                ),
                _ => None,
            })
            .collect()
    }

    pub fn remove_node(&mut self, node_id: SceneNodeId) -> Option<SceneNode> {
        let index = self.nodes.iter().position(|node| node.id == node_id)?;
        let removed = self.nodes.remove(index);

        if let Some(parent_id) = removed.parent {
            if let Some(parent) = self.node_mut(parent_id) {
                parent.children.retain(|child_id| *child_id != node_id);
            }
        }

        let child_ids = removed.children.clone();
        for child_id in child_ids {
            let _ = self.remove_node(child_id);
        }

        Some(removed)
    }

    fn add_node(
        &mut self,
        parent: Option<SceneNodeId>,
        kind: SceneNodeKind,
        geometry: WindowGeometry,
    ) -> SceneNodeId {
        let id = SceneNodeId::new(self.next_node_id);
        self.next_node_id += 1;

        self.nodes.push(SceneNode::new(id, parent, kind, geometry));

        if let Some(parent_id) = parent {
            if let Some(parent) = self.node_mut(parent_id) {
                parent.children.push(id);
            }
        }

        println!("Scene node created");

        id
    }

    fn node_mut(&mut self, node_id: SceneNodeId) -> Option<&mut SceneNode> {
        self.nodes.iter_mut().find(|node| node.id == node_id)
    }
}
