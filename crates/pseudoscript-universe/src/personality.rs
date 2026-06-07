//! Planet personalities — the universe's *character*, derived from the language's
//! own macros, a node's tags, and how busy it is. Every world is one of a few
//! [`Archetype`]s (a star, a storm, a tomb…), with a **vitality** (thriving vs
//! decaying), a **mass** (importance), and a **heat** (how much it glows). Pure
//! derivation from model signals plus optional freshness; the renderer turns these
//! into colour, surface, atmosphere, and motion.
//!
//! The signals are real, not invented:
//! - `#[schedule]` callables beat on a rhythm → a **Pulsar**.
//! - `#[onevent(..)]` callables react → a **Storm**.
//! - `#[http(..)]` callables face the outside → a **Gateway**.
//! - `#[manual]` callables are hand-driven → a **Forge**.
//! - a `#headline` / `#critical` tag → a luminous **Beacon**.
//! - a system is the **Star** its containers orbit.
//! - idle *and* unreachable → a frozen **Tomb**, slowly decaying.

use pseudoscript_model::NodeKind;

/// The kind of world a node is — its visual and behavioural personality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    /// A system — the sun a solar system's containers orbit.
    Star,
    /// Marked `#headline` / `#critical` — a luminous giant.
    Beacon,
    /// Scheduled work — pulses on a rhythm.
    Pulsar,
    /// Event-driven — reactive, storm-wracked.
    Storm,
    /// HTTP-facing — exposed to the outside, lit from without.
    Gateway,
    /// Manual / human-driven — warm, hand-worked.
    Forge,
    /// An ordinary container or component.
    World,
    /// Unreachable and idle — frozen, overgrown, decaying.
    Tomb,
}

/// A node's personality: what kind of world, and how alive it is.
#[derive(Debug, Clone)]
pub struct Planet {
    pub archetype: Archetype,
    /// 0 = decaying (old, idle, unreachable) … 1 = thriving (fresh, busy, central).
    pub vitality: f32,
    /// 0 … 1 importance — drives size and gravitational pull.
    pub mass: f32,
    /// 0 … 1 activity — drives glow, particle emission, atmosphere.
    pub heat: f32,
    /// The node's own tags (`#headline`, `#critical`, …), passed through for the
    /// renderer to badge.
    pub tags: Vec<String>,
}

impl Default for Planet {
    fn default() -> Self {
        Self {
            archetype: Archetype::World,
            vitality: 0.5,
            mass: 0.3,
            heat: 0.0,
            tags: Vec::new(),
        }
    }
}

/// What a structural node accumulates from itself and its callables, before it is
/// classified into a [`Planet`].
#[derive(Debug, Default)]
pub struct Signals {
    /// Trigger histogram over the node's descendant callables.
    pub scheduled: u32,
    pub events: u32,
    pub http: u32,
    pub manual: u32,
    /// Relationship edges touching this node (in + out), summed by traffic.
    pub traffic: u32,
    /// Containment children (containers in a system, components in a container).
    pub children: u32,
    /// The node's own doc tags.
    pub tags: Vec<String>,
    /// Recency in `[0, 1]` (1 = just modified), when the host supplies it.
    pub freshness: Option<f32>,
}

impl Signals {
    fn triggers(&self) -> u32 {
        self.scheduled + self.events + self.http + self.manual
    }

    /// Whether any tag marks this node as a headline element (case-insensitively,
    /// with or without a leading `#`).
    fn is_beacon(&self) -> bool {
        self.tags.iter().any(|t| {
            let t = t.trim_start_matches('#');
            t.eq_ignore_ascii_case("headline") || t.eq_ignore_ascii_case("critical")
        })
    }
}

#[allow(clippy::cast_precision_loss)]
fn saturate(n: u32, scale: f32) -> f32 {
    (n as f32 / scale).clamp(0.0, 1.0)
}

/// Classify a structural node into a [`Planet`] from its kind and signals.
#[must_use]
pub fn classify(kind: NodeKind, sig: &Signals) -> Planet {
    let triggers = sig.triggers();

    let archetype = if kind == NodeKind::System {
        Archetype::Star
    } else if sig.is_beacon() {
        Archetype::Beacon
    } else if triggers == 0 && sig.traffic == 0 {
        Archetype::Tomb
    } else if triggers > 0 {
        // The loudest trigger sets the temperament; ties resolve in this order.
        let loudest = [
            (sig.scheduled, Archetype::Pulsar),
            (sig.events, Archetype::Storm),
            (sig.http, Archetype::Gateway),
            (sig.manual, Archetype::Forge),
        ]
        .into_iter()
        .max_by_key(|(n, _)| *n);
        loudest.map_or(Archetype::World, |(_, a)| a)
    } else {
        Archetype::World
    };

    // Activity drives heat and the no-freshness vitality fallback.
    let activity = saturate(triggers + sig.traffic, 8.0);
    let mass = (saturate(sig.traffic, 6.0) * 0.5
        + saturate(sig.children, 6.0) * 0.3
        + if sig.is_beacon() { 0.2 } else { 0.0 })
    .clamp(0.0, 1.0);
    let heat = saturate(triggers * 2 + sig.traffic, 10.0);

    // Thriving vs decaying: freshness when known, else activity. A Tomb is dragged
    // further down so abandoned corners read as truly dead.
    let base = sig.freshness.map_or(activity, |f| 0.5 * f + 0.5 * activity);
    let vitality = if archetype == Archetype::Tomb {
        base * 0.3
    } else {
        base
    }
    .clamp(0.0, 1.0);

    Planet {
        archetype,
        vitality,
        mass,
        heat,
        tags: sig.tags.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig() -> Signals {
        Signals::default()
    }

    #[test]
    fn a_system_is_a_star() {
        assert_eq!(
            classify(NodeKind::System, &sig()).archetype,
            Archetype::Star
        );
    }

    #[test]
    fn loudest_trigger_sets_the_temperament() {
        let s = Signals {
            events: 3,
            http: 1,
            traffic: 2,
            ..sig()
        };
        assert_eq!(
            classify(NodeKind::Component, &s).archetype,
            Archetype::Storm
        );
    }

    #[test]
    fn a_headline_tag_makes_a_beacon_even_when_busy() {
        let s = Signals {
            scheduled: 5,
            tags: vec!["headline".into()],
            ..sig()
        };
        assert_eq!(
            classify(NodeKind::Container, &s).archetype,
            Archetype::Beacon
        );
    }

    #[test]
    fn idle_and_unreachable_is_a_decaying_tomb() {
        let p = classify(NodeKind::Component, &sig());
        assert_eq!(p.archetype, Archetype::Tomb);
        assert!(p.vitality < 0.2, "a tomb decays: {}", p.vitality);
    }

    #[test]
    fn freshness_lifts_vitality() {
        let cold = Signals {
            http: 1,
            traffic: 1,
            freshness: Some(0.0),
            ..sig()
        };
        let warm = Signals {
            freshness: Some(1.0),
            ..cold_clone(&cold)
        };
        assert!(
            classify(NodeKind::Container, &warm).vitality
                > classify(NodeKind::Container, &cold).vitality
        );
    }

    fn cold_clone(s: &Signals) -> Signals {
        Signals {
            http: s.http,
            traffic: s.traffic,
            ..Signals::default()
        }
    }
}
