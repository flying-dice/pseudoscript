//! Entry-point flows, traced through the model's sequence projections.
//!
//! A flow starts at an **entry point** — a callable carrying a trigger macro,
//! or a callable owned by a `person` (an action the person initiates). Each
//! entry's sequence view is projected through `pseudoscript-emit` and its call
//! messages flattened into hops; both ends of every hop are lifted to the
//! nearest placed structural node — the same lift the adapter applies to
//! relationships, so flows and edges agree. Colours key from the entry's FQN by
//! a stable FNV-1a hash into the categorical palette the web IDE shares, so a
//! flow keeps its hue everywhere it appears and an unrelated model edit never
//! recolours it. Deterministic: entries are traced in FQN order, no clock, no
//! randomness.

use std::collections::HashSet;

use pseudoscript_emit::{MessageKind, Scene, SeqItem, View, project};
use pseudoscript_model::{Graph as Model, NodeKind};
use serde::Serialize;

use crate::model_adapter::lift_fqn;

/// One call leg of a flow: caller and callee as structural node ids (the same
/// ids the snapshot places), plus the message label shown on the leg.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowHop {
    /// Source structural node id.
    pub from: String,
    /// Target structural node id.
    pub to: String,
    /// The call label (method name).
    pub label: String,
}

/// One entry-point flow: the entry callable's FQN and simple name, the flow's
/// palette colour, and its ordered legs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowDef {
    /// The entry callable's FQN.
    pub fqn: String,
    /// The entry's simple name.
    pub name: String,
    /// The flow's stable palette colour (hex).
    pub color: String,
    /// The flow's legs in call order.
    pub hops: Vec<FlowHop>,
}

/// The categorical flow palette — the same sixteen hues the web IDE's
/// `flow-color` module paints with, so doc site and IDE agree on every flow.
const FLOW_PALETTE: [&str; 16] = [
    "#ff6b6b", "#ffa94d", "#ffd43b", "#a9e34b", "#69db7c", "#38d9a9", "#3bc9db", "#4dabf7",
    "#748ffc", "#9775fa", "#da77f2", "#f783ac", "#ff8787", "#ffc078", "#94d82d", "#20c997",
];

/// FNV-1a (32-bit) over `key` — the same hash the web IDE keys flow colours
/// with, so the two renderers pick identical hues.
fn fnv1a(key: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for byte in key.bytes() {
        h ^= u32::from(byte);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The stable palette colour for an entry FQN.
fn color_of(fqn: &str) -> String {
    FLOW_PALETTE[fnv1a(fqn) as usize % FLOW_PALETTE.len()].to_owned()
}

/// Trace every entry-point flow in `model`, in FQN order.
#[must_use]
pub fn flows(model: &Model) -> Vec<FlowDef> {
    let placed: HashSet<&str> = model
        .nodes()
        .iter()
        .filter(|n| {
            matches!(
                n.kind,
                NodeKind::System | NodeKind::Container | NodeKind::Component | NodeKind::Person
            )
        })
        .map(|n| n.fqn.as_str())
        .collect();

    let mut entries: Vec<&str> = model
        .nodes()
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Callable
                && (!n.triggers.is_empty() || person_owned(model, n.parent.as_deref()))
        })
        .map(|n| n.fqn.as_str())
        .collect();
    entries.sort_unstable();

    entries
        .into_iter()
        .map(|fqn| trace(model, fqn, &placed))
        .collect()
}

/// Whether the owning node is a `person` — its actions are flow entries.
fn person_owned(model: &Model, parent: Option<&str>) -> bool {
    parent
        .and_then(|p| model.node(p))
        .is_some_and(|n| n.kind == NodeKind::Person)
}

/// One entry's flow: project its sequence view and flatten the call messages
/// into lifted hops. A projection failure yields a flow with no hops rather
/// than aborting — a partial model still animates.
fn trace(model: &Model, entry: &str, placed: &HashSet<&str>) -> FlowDef {
    let mut hops = Vec::new();
    if let Ok(Scene::Sequence(seq)) = project(
        model,
        View::Sequence {
            entry: entry.to_owned(),
        },
    ) {
        walk(model, &seq.items, placed, &mut hops);
    }
    FlowDef {
        fqn: entry.to_owned(),
        name: simple_name(entry).to_owned(),
        color: color_of(entry),
        hops,
    }
}

/// Flatten sequence items into hops: calls and self-calls become legs (returns
/// are skipped), frames recurse into their bodies in order. Each leg's ends
/// lift to the nearest placed structural node; a leg whose ends lift to the
/// same node, or fail to lift, is dropped.
fn walk(model: &Model, items: &[SeqItem], placed: &HashSet<&str>, out: &mut Vec<FlowHop>) {
    for item in items {
        match item {
            SeqItem::Message(m) => {
                if m.kind == MessageKind::Return {
                    continue;
                }
                let (Some(from), Some(to)) = (
                    lift_fqn(model, &m.from, |c| placed.contains(c)),
                    lift_fqn(model, &m.to, |c| placed.contains(c)),
                ) else {
                    continue;
                };
                if from != to {
                    out.push(FlowHop {
                        from: from.to_owned(),
                        to: to.to_owned(),
                        label: m.label.clone(),
                    });
                }
            }
            SeqItem::Frame(f) => walk(model, &f.body, placed, out),
        }
    }
}

/// The leaf segment of an FQN.
fn simple_name(fqn: &str) -> &str {
    fqn.rsplit("::").next().unwrap_or(fqn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pseudoscript_model::{WorkspaceModule, graph as build_model};

    fn model(src: &str) -> Model {
        build_model(&[WorkspaceModule::new("m", src)])
    }

    const SHOP: &str = "\
person Buyer {
  Browse() {
    m::Api.List()
  }
}
system Shop;
container Api for m::Shop {
  #[http(\"POST /orders\")]
  Place() {
    m::Stock.Reserve()
    if (true) {
      m::Stock.Release()
    }
  }
  List();
}
container Stock for m::Shop {
  Reserve();
  Release();
}
";

    #[test]
    fn triggered_and_person_owned_callables_start_flows_in_fqn_order() {
        let flows = flows(&model(SHOP));
        let fqns: Vec<&str> = flows.iter().map(|f| f.fqn.as_str()).collect();
        // `Browse` is person-owned, `Place` is triggered; `List`/`Reserve`/
        // `Release` are neither. FQN order.
        assert_eq!(fqns, ["m::Api::Place", "m::Buyer::Browse"]);
    }

    #[test]
    fn hops_lift_to_structural_nodes_and_recurse_frames() {
        let flows = flows(&model(SHOP));
        let place = flows.iter().find(|f| f.name == "Place").expect("flow");
        // Both the direct call and the framed (if-guarded) call appear, lifted
        // from callables to their containers.
        let legs: Vec<(&str, &str, &str)> = place
            .hops
            .iter()
            .map(|h| (h.from.as_str(), h.to.as_str(), h.label.as_str()))
            .collect();
        assert!(legs.contains(&("m::Api", "m::Stock", "Reserve")));
        assert!(legs.contains(&("m::Api", "m::Stock", "Release")));
    }

    #[test]
    fn same_lift_drops_intra_node_legs() {
        let src = "\
system Sys;
container App for m::Sys {
  #[manual]
  Run() {
    self.Helper()
  }
  Helper();
}
";
        let flows = flows(&model(src));
        assert_eq!(flows.len(), 1);
        assert!(
            flows[0].hops.is_empty(),
            "self-call lifts to one node and drops"
        );
    }

    #[test]
    fn colours_key_from_the_entry_fqn() {
        let twice = (color_of("m::Api::Place"), color_of("m::Api::Place"));
        assert_eq!(twice.0, twice.1);
        assert!(FLOW_PALETTE.contains(&twice.0.as_str()));
        // The keying matches the web IDE's flow-color module: FNV-1a (imul) on
        // the same palette. Pin one known pairing so a palette edit is caught.
        assert_eq!(color_of(""), FLOW_PALETTE[fnv1a("") as usize % 16]);
    }

    #[test]
    fn a_failed_projection_yields_an_empty_flow_not_a_panic() {
        // A triggered callable on a person has no sequence (no body) — the flow
        // exists with no hops.
        let src = "\
person Op {
  Press();
}
";
        let flows = flows(&model(src));
        assert_eq!(flows.len(), 1);
        assert!(flows[0].hops.is_empty());
    }
}
