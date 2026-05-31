//! Acceptance tests for the layout engines. Diagrams are described as JSON in
//! Gherkin docstrings and laid out with default metrics; the assertions are
//! geometric invariants the engine must hold for any input — no dependency on
//! the rest of the compiler, proving the crate stands alone.

use std::path::Path;

use cucumber::{World, gherkin::Step, given, then, when};
use pseudoscript_layout::sequence::layout::{Layout, PlacedParticipant};
use pseudoscript_layout::sequence::{Diagram, Message, Metrics, MsgKind, layout};

#[derive(Debug, Default, World)]
struct LayoutWorld {
    diagram: Option<Diagram>,
    metrics: Metrics,
    layout: Option<Layout>,
}

impl LayoutWorld {
    fn out(&self) -> &Layout {
        self.layout.as_ref().expect("a laid-out diagram")
    }

    fn participant(&self, id: &str) -> &PlacedParticipant {
        self.out()
            .participants
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("no participant {id:?}"))
    }
}

#[given(regex = r"^the sequence diagram:$")]
fn given_diagram(world: &mut LayoutWorld, step: &Step) {
    let json = step.docstring().expect("a JSON diagram docstring");
    let diagram: Diagram = serde_json::from_str(json).expect("valid diagram JSON");
    world.diagram = Some(diagram);
}

#[when("it is laid out")]
fn lay_out(world: &mut LayoutWorld) {
    let diagram = world.diagram.as_ref().expect("a given diagram");
    world.layout = Some(layout(diagram, &world.metrics));
}

#[then("no participant cards overlap")]
fn cards_do_not_overlap(world: &mut LayoutWorld) {
    let cards = &world.out().participants;
    for pair in cards.windows(2) {
        assert!(
            pair[0].card.right() <= pair[1].card.x,
            "{} card (right {}) overlaps {} card (left {})",
            pair[0].id,
            pair[0].card.right(),
            pair[1].id,
            pair[1].card.x,
        );
    }
}

#[then("every message label fits its lane")]
fn labels_fit(world: &mut LayoutWorld) {
    let out = world.out();
    for m in &out.messages {
        let pill = world.metrics.message_pill_width(&Message {
            from: m.from.clone(),
            to: m.to.clone(),
            kind: m.kind,
            label: m.label.clone(),
            detail: m.detail.clone(),
        });
        if m.kind == MsgKind::SelfMsg {
            // A self label sits to the right of its lifeline; it must fit before
            // the canvas edge.
            let right = m.from_x + world.metrics.self_offset + pill;
            assert!(
                right <= out.width,
                "self label {:?} overruns canvas",
                m.label
            );
        } else if m.from != m.to {
            let from_x = world.participant(&m.from).lifeline_x;
            let to_x = world.participant(&m.to).lifeline_x;
            let avail = (from_x - to_x).abs();
            assert!(
                pill <= avail,
                "label {:?} ({pill}px) wider than its {avail}px lane",
                m.label,
            );
        }
    }
}

#[then("every fragment box has ordered interior dividers")]
fn fragments_well_formed(world: &mut LayoutWorld) {
    for (i, f) in world.out().fragments.iter().enumerate() {
        assert!(f.rect.w > 0 && f.rect.h > 0, "fragment {i} has empty box");
        let mut last = f.rect.y;
        for d in &f.dividers {
            assert!(
                d.y > f.rect.y && d.y < f.rect.bottom(),
                "fragment {i} divider {} outside its box",
                d.y,
            );
            assert!(d.y > last, "fragment {i} dividers not ordered");
            last = d.y;
        }
    }
}

// Cucumber binds regex captures as owned `String`; the by-value params are the
// step macro's contract, not a smell.
#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^the activation for "([^"]+)" covers its messages$"#)]
fn activation_covers(world: &mut LayoutWorld, id: String) {
    let out = world.out();
    let act = out
        .activations
        .iter()
        .find(|a| a.participant == id)
        .unwrap_or_else(|| panic!("no activation for {id:?}"));
    let ys: Vec<i32> = out
        .messages
        .iter()
        .filter(|m| m.from == id || m.to == id)
        .map(|m| m.y)
        .collect();
    assert!(!ys.is_empty(), "{id:?} has no messages");
    let lo = *ys.iter().min().unwrap();
    let hi = *ys.iter().max().unwrap();
    assert!(
        act.top <= lo && act.bottom >= hi,
        "{id:?} activation [{},{}] does not cover messages [{lo},{hi}]",
        act.top,
        act.bottom,
    );
}

#[allow(clippy::needless_pass_by_value)]
#[then(regex = r#"^fragment ([0-9]+) splits "([^"]+)" above "([^"]+)"$"#)]
fn fragment_splits(world: &mut LayoutWorld, idx: usize, above: String, below: String) {
    let out = world.out();
    let f = &out.fragments[idx];
    assert_eq!(f.dividers.len(), 1, "expected one divider");
    let div = f.dividers[0].y;
    let y_of = |label: &str| {
        out.messages
            .iter()
            .find(|m| m.label == label)
            .unwrap_or_else(|| panic!("no message {label:?}"))
            .y
    };
    let a = y_of(&above);
    let b = y_of(&below);
    assert!(
        a < div && div < b,
        "{above:?} ({a}) / divider ({div}) / {below:?} ({b}) not split"
    );
    assert!(
        f.rect.y <= a && b <= f.rect.bottom(),
        "fragment box does not enclose both branches",
    );
}

fn main() {
    futures::executor::block_on(LayoutWorld::run(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features"),
    ));
}
