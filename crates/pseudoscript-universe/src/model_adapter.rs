//! Adapt the resolved `PseudoScript` C4 model into the universe's internal graph.
//!
//! The universe places only the **structural** nodes — systems, containers,
//! components. Persons, data, and callables are not placed; a relationship that
//! runs through them is *lifted* to the nearest enclosing structural node, so the
//! graph carries node-to-node relationships at any level (spec §4, §7). Containment
//! lives on each node (`parent`/`children`); relationships are the petgraph edges.
//! All positions are engine outputs — zero here, filled by the simulation later.

use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};
use pseudoscript_model::{
    EdgeKind, Graph as Model, NodeKind, Trigger, WorkspaceModule, graph as build_model,
};

use crate::personality::{Planet, Signals, classify};

/// Per-module recency in `[0, 1]` (1 = just modified), keyed by module FQN — the
/// host supplies it (file mtimes / git) to drive thriving-vs-decaying vitality.
pub type Freshness = HashMap<String, f32>;

/// An index into the universe graph (petgraph's node index).
pub type NodeIx = NodeIndex<u32>;

/// The C4 abstraction level a placed node sits at. Systems, containers, components,
/// and the people (actors) who use them enter the universe; data and callables do not
/// (relationships through them are lifted to the nearest of these).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4Level {
    System,
    Container,
    Component,
    Person,
}

/// A node in the software graph: a system, container, component, or person, with its
/// place in the containment tree and its macro-derived personality. Positions are not
/// here — the renderer lays the graph out client-side.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    /// Stable model FQN.
    pub id: String,
    pub level: C4Level,
    /// Enclosing node in the containment tree (`None` for a top-level system).
    pub parent: Option<NodeIx>,
    pub children: Vec<NodeIx>,
    /// The node's character — archetype, vitality, mass, heat (see [`Planet`]).
    pub planet: Planet,
}

impl LayoutNode {
    fn new(id: String, level: C4Level) -> Self {
        Self { id, level, parent: None, children: Vec::new(), planet: Planet::default() }
    }
}

/// The software graph: structural nodes (petgraph weights) with the containment tree
/// on each node, directed relationship edges (weighted by traffic) between them, and
/// the top-level systems.
pub struct Universe {
    /// Nodes are [`LayoutNode`]; each edge weight is its **traffic** — the number
    /// of underlying calls between the two nodes.
    pub graph: Graph<LayoutNode, u32>,
    /// Top-level systems (containment roots), in model declaration order.
    pub roots: Vec<NodeIx>,
}

/// Build the universe from module sources — the same `(fqn, source)` input the IDE
/// feeds [`pseudoscript_model::graph`]. No freshness, so vitality is activity-only.
#[must_use]
pub fn build(modules: &[WorkspaceModule]) -> Universe {
    from_model_with(&build_model(modules), None)
}

/// As [`build`], with per-module `freshness` driving thriving-vs-decaying vitality.
#[must_use]
pub fn build_with(modules: &[WorkspaceModule], freshness: &Freshness) -> Universe {
    from_model_with(&build_model(modules), Some(freshness))
}

/// Adapt an already-resolved model into the universe graph (activity-only vitality).
#[must_use]
pub fn from_model(model: &Model) -> Universe {
    from_model_with(model, None)
}

/// Adapt a resolved model into the universe graph, computing each node's
/// personality from the language's macros, its tags, traffic, and (optionally)
/// recency.
#[must_use]
pub fn from_model_with(model: &Model, freshness: Option<&Freshness>) -> Universe {
    let mut graph: Graph<LayoutNode, u32> = Graph::new();

    // 1. Each structural model node becomes a universe node; remember its index.
    let mut ix: HashMap<&str, NodeIx> = HashMap::new();
    for node in model.nodes() {
        if let Some(level) = level_of(node.kind) {
            let nx = graph.add_node(LayoutNode::new(node.fqn.clone(), level));
            ix.insert(node.fqn.as_str(), nx);
        }
    }

    // 2. Containment: hook each node to its structural parent (collected first so
    //    the graph isn't borrowed mutably while reading model parent links).
    let links: Vec<(NodeIx, Option<NodeIx>)> = model
        .nodes()
        .iter()
        .filter_map(|node| {
            let nx = *ix.get(node.fqn.as_str())?;
            Some((nx, node.parent.as_deref().and_then(|p| ix.get(p).copied())))
        })
        .collect();
    let mut roots = Vec::new();
    for (nx, parent) in links {
        graph[nx].parent = parent;
        match parent {
            Some(p) => graph[p].children.push(nx),
            None => roots.push(nx),
        }
    }

    // 3. Relationships: lift every call to the structural level and tally traffic per
    //    *directed* pair (caller → callee), so the edge carries direction — the flow
    //    animation in the renderer streams along it. The count becomes the weight.
    let mut traffic: HashMap<(NodeIx, NodeIx), u32> = HashMap::new();
    for edge in model.edges() {
        if edge.kind != EdgeKind::Call {
            continue;
        }
        let (Some(a), Some(b)) = (lift(model, &edge.from, &ix), lift(model, &edge.to, &ix))
        else {
            continue;
        };
        if a != b {
            *traffic.entry((a, b)).or_default() += 1;
        }
    }
    // Add edges in a stable order (HashMap iteration is randomised) so the force
    // sim's float accumulation is identical every run — determinism (spec §8).
    let mut pairs: Vec<((NodeIx, NodeIx), u32)> = traffic.iter().map(|(&k, &v)| (k, v)).collect();
    pairs.sort_by_key(|((a, b), _)| (a.index(), b.index()));
    for ((a, b), weight) in pairs {
        graph.add_edge(a, b, weight);
    }

    // 4. Personality: aggregate each node's signals, then classify it into a planet.
    let signals = collect_signals(model, &ix, &graph, freshness);
    for node in model.nodes() {
        if let Some(&nx) = ix.get(node.fqn.as_str()) {
            graph[nx].planet = classify(node.kind, &signals[nx.index()]);
        }
    }

    Universe { graph, roots }
}

/// Gather, per universe node, what classifies its personality: its own tags and
/// children, the trigger macros of every callable it contains (lifted up), the
/// traffic on its edges, and its module's freshness.
fn collect_signals(
    model: &Model,
    ix: &HashMap<&str, NodeIx>,
    graph: &Graph<LayoutNode, u32>,
    freshness: Option<&Freshness>,
) -> Vec<Signals> {
    let mut signals: Vec<Signals> = (0..graph.node_count()).map(|_| Signals::default()).collect();

    for node in model.nodes() {
        match node.kind {
            // A structural node contributes its own tags, children, and freshness.
            NodeKind::System | NodeKind::Container | NodeKind::Component => {
                if let Some(&nx) = ix.get(node.fqn.as_str()) {
                    let s = &mut signals[nx.index()];
                    s.tags.clone_from(&node.doc.tags);
                    s.children = u32::try_from(graph[nx].children.len()).unwrap_or(u32::MAX);
                    s.freshness = freshness.and_then(|f| f.get(&node.module).copied());
                }
            }
            // A callable's triggers belong to the structural node enclosing it.
            NodeKind::Callable if !node.triggers.is_empty() => {
                if let Some(anc) = lift(model, &node.fqn, ix) {
                    let s = &mut signals[anc.index()];
                    for trigger in &node.triggers {
                        match trigger {
                            Trigger::Schedule => s.scheduled += 1,
                            Trigger::OnEvent { .. } => s.events += 1,
                            Trigger::Http => s.http += 1,
                            Trigger::Manual => s.manual += 1,
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for e in graph.edge_indices() {
        if let Some((a, b)) = graph.edge_endpoints(e) {
            let w = graph[e];
            signals[a.index()].traffic += w;
            signals[b.index()].traffic += w;
        }
    }
    signals
}

/// The structural level of a model kind, or `None` for the kinds the universe does
/// not place (person, data, callable).
fn level_of(kind: NodeKind) -> Option<C4Level> {
    match kind {
        NodeKind::System => Some(C4Level::System),
        NodeKind::Container => Some(C4Level::Container),
        NodeKind::Component => Some(C4Level::Component),
        NodeKind::Person => Some(C4Level::Person),
        NodeKind::Data | NodeKind::Callable => None,
    }
}

/// Walk a model FQN up its `parent` chain to the nearest structural node present
/// in the universe; `None` if the chain reaches the top without one.
fn lift(model: &Model, fqn: &str, ix: &HashMap<&str, NodeIx>) -> Option<NodeIx> {
    let mut cur = fqn;
    loop {
        if let Some(&nx) = ix.get(cur) {
            return Some(nx);
        }
        cur = model.node(cur)?.parent.as_deref()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::Archetype;

    fn module(src: &str) -> Vec<WorkspaceModule> {
        vec![WorkspaceModule::new("m", src)]
    }

    #[test]
    fn places_structural_nodes_with_containment_and_lifts_calls() {
        // One system with two containers; a call from App's body into Svc.
        let src = "\
system Sys;
container App for m::Sys {
  Do() {
    m::Svc.Handle()
  }
}
container Svc for m::Sys {
  Handle();
}
";
        let u = build(&module(src));

        // Only the three structural nodes are placed (the callables are not).
        assert_eq!(u.graph.node_count(), 3, "system + two containers");

        // The system is the sole containment root, holding both containers.
        assert_eq!(u.roots.len(), 1);
        let sys = u.roots[0];
        assert_eq!(u.graph[sys].level, C4Level::System);
        assert_eq!(u.graph[sys].children.len(), 2);

        // The call App.Do -> Svc.Handle lifts to a single App<->Svc relationship.
        assert_eq!(u.graph.edge_count(), 1);

        // Every placed node's level is structural, and ids are the model FQNs.
        for nx in u.graph.node_indices() {
            let n = &u.graph[nx];
            assert!(matches!(n.level, C4Level::System | C4Level::Container));
            assert!(n.id.starts_with("m::"));
        }
    }

    #[test]
    fn an_empty_model_yields_an_empty_universe() {
        let u = build(&[]);
        assert_eq!(u.graph.node_count(), 0);
        assert!(u.roots.is_empty());
    }

    #[test]
    fn personality_flows_from_macros_tags_and_traffic() {
        // A hand-driven container calls a hub; one container is a headline; one is
        // never touched. The macros/tags/connectivity become planet personalities.
        let src = "\
system Sys;
container Hands for m::Sys {
  #[manual]
  Do() { m::Hub.Take() }
}
container Hub for m::Sys { Take(); }
/// #headline
container Crown for m::Sys { Shine(); }
container Lonely for m::Sys { Idle(); }
";
        let u = build(&module(src));
        let planet = |name: &str| {
            let nx = u
                .graph
                .node_indices()
                .find(|&nx| u.graph[nx].id.ends_with(name))
                .unwrap_or_else(|| panic!("missing {name}"));
            u.graph[nx].planet.clone()
        };

        assert_eq!(planet("Sys").archetype, Archetype::Star);
        assert_eq!(planet("Hands").archetype, Archetype::Forge, "#[manual] lifts to a Forge");
        assert_eq!(planet("Crown").archetype, Archetype::Beacon, "#headline → Beacon");
        // Hub receives traffic but runs no macros — an ordinary, living World.
        assert_eq!(planet("Hub").archetype, Archetype::World);
        assert!(planet("Hub").vitality > 0.0);
        // Lonely is idle and unreachable — a decaying Tomb.
        let lonely = planet("Lonely");
        assert_eq!(lonely.archetype, Archetype::Tomb);
        assert!(lonely.vitality < 0.2, "a tomb decays: {}", lonely.vitality);

        // The Hands → Hub call is the only relationship; weight 1 (one call).
        assert_eq!(u.graph.edge_count(), 1);
        assert_eq!(u.graph.edge_weights().copied().max(), Some(1));
    }
}
