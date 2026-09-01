//! What changed, told as invalidation rather than as data.
//!
//! Every few seconds a screen asks "what has changed since my mark?" and gets
//! back a list of *things to go and re-read* — never the rows themselves. The
//! screen then re-fetches through exactly the read paths it always uses, so
//! every grant and every rule is checked again on the way. That is the whole
//! point: if news carried data, it would be a second way to read the database,
//! and a second way to read the database is a second place for a permission
//! check to drift out of step with the first.
//!
//! Three rules that are easy to get wrong:
//!
//! - **Only successes are news.** A refused write changed nothing, so it
//!   announces nothing.
//! - **A renewal is silent.** A heartbeat every twenty seconds per open editor
//!   would drown the log in events that mean "still, yes".
//! - **A person hears only about areas they may read — but their mark still
//!   advances past the rest.** Otherwise someone without the Admin grant would
//!   be permanently stuck behind the first admin event and re-fetch forever.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;
use uuid::Uuid;

use super::registry::{decode, encode, BoundaryCtx, Registry};
use super::{Access, Actor, Area, BoundaryError, Required};

/// How often a screen should ask. Exported so the frontend and the docs cannot
/// disagree about it.
pub const NEWS_BEAT_SECONDS: u64 = 5;

/// How much history is kept. A client further behind than this is told to start
/// over rather than handed a partial picture.
pub const NEWS_CAPACITY: usize = 1000;

/// One thing that happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notice {
    /// A row changed. Go and re-read it.
    RecordChanged {
        area: Area,
        record_kind: String,
        record_id: String,
    },
    /// Someone took an edit hold — draw their name on it.
    RecordBusy {
        area: Area,
        record_kind: String,
        record_id: String,
        holder: String,
    },
    /// A hold was given up — take the badge down.
    RecordFreed {
        area: Area,
        record_kind: String,
        record_id: String,
    },
    /// The host closed the file for maintenance.
    MaintenanceOn { closed_by: String },
    /// The host reopened it.
    MaintenanceOff,
}

impl Notice {
    /// Which area a person must be able to read to hear this.
    ///
    /// `None` means everyone hears it: a closed sign is not an area-scoped
    /// fact, and someone who cannot hear it would sit wondering why their
    /// saves are being refused.
    pub fn area(&self) -> Option<Area> {
        match self {
            Notice::RecordChanged { area, .. }
            | Notice::RecordBusy { area, .. }
            | Notice::RecordFreed { area, .. } => Some(*area),
            Notice::MaintenanceOn { .. } | Notice::MaintenanceOff => None,
        }
    }

    fn audible_to(&self, actor: &Actor) -> bool {
        match self.area() {
            None => true,
            Some(area) => actor.grants().access(area) >= Access::Read,
        }
    }
}

/// A client's place in the log.
///
/// Carries the run it came from as well as the sequence. A mark from a previous
/// run of the host is not merely old — the sequence numbers it refers to mean
/// something different now — so it is rejected the same way an over-old mark
/// is. That one behaviour gives re-sync after a host restart for free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub run: String,
    pub seq: u64,
}

/// The answer to "what changed?".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CatchUp {
    /// Here is what you missed, and your new mark.
    Notices { notices: Vec<Notice>, mark: Mark },
    /// You are too far behind, or you are holding a mark from a previous run.
    /// Re-read everything on screen and continue from this mark.
    StartOver { mark: Mark },
}

#[derive(Debug)]
pub struct News {
    run: String,
    inner: Mutex<Ring>,
}

#[derive(Debug)]
struct Ring {
    entries: VecDeque<(u64, Notice)>,
    next_seq: u64,
}

impl Default for News {
    fn default() -> Self {
        News::new()
    }
}

impl News {
    pub fn new() -> Self {
        News {
            run: Uuid::new_v4().to_string(),
            inner: Mutex::new(Ring {
                entries: VecDeque::new(),
                next_seq: 1,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The mark a client should start from when it has no history — the
    /// present moment, so it hears about what happens next but is not handed
    /// a backlog it has no use for.
    pub fn current_mark(&self) -> Mark {
        let ring = self.lock();
        Mark {
            run: self.run.clone(),
            seq: ring.next_seq.saturating_sub(1),
        }
    }

    /// Record something that happened.
    ///
    /// Call this only after the thing actually succeeded.
    pub fn publish(&self, notice: Notice) {
        let mut ring = self.lock();
        let seq = ring.next_seq;
        ring.next_seq += 1;
        ring.entries.push_back((seq, notice));
        while ring.entries.len() > NEWS_CAPACITY {
            ring.entries.pop_front();
        }
    }

    /// Everything after `mark` that this actor is allowed to hear.
    pub fn catch_up(&self, mark: &Mark, actor: &Actor) -> CatchUp {
        let ring = self.lock();
        let newest = ring.next_seq.saturating_sub(1);

        // A mark from another run refers to sequence numbers that no longer
        // mean anything. Handled here, on the same path as being too far
        // behind, so a host restart re-syncs every client without a special
        // case anywhere.
        if mark.run != self.run {
            return CatchUp::StartOver {
                mark: Mark {
                    run: self.run.clone(),
                    seq: newest,
                },
            };
        }

        let oldest = ring.entries.front().map(|(seq, _)| *seq);
        if let Some(oldest) = oldest {
            // They want everything after mark.seq; the earliest we still hold
            // is `oldest`. If a gap opened between them, we cannot honestly
            // fill it.
            if mark.seq + 1 < oldest {
                return CatchUp::StartOver {
                    mark: Mark {
                        run: self.run.clone(),
                        seq: newest,
                    },
                };
            }
        }

        let notices: Vec<Notice> = ring
            .entries
            .iter()
            .filter(|(seq, _)| *seq > mark.seq)
            .filter(|(_, notice)| notice.audible_to(actor))
            .map(|(_, notice)| notice.clone())
            .collect();

        // Note the mark advances to `newest` regardless of what was filtered
        // out. Someone who cannot read an area must still move past its
        // events, or they would ask about the same ones forever.
        CatchUp::Notices {
            notices,
            mark: Mark {
                run: self.run.clone(),
                seq: newest,
            },
        }
    }

    #[cfg(test)]
    fn held(&self) -> usize {
        self.lock().entries.len()
    }
}

// -------------------------------------------------------------- commands

#[derive(Debug, Deserialize)]
struct CatchUpArgs {
    mark: Option<Mark>,
}

fn h_catch_up(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: CatchUpArgs = decode(args)?;
    let news = &ctx.shared.news;

    // No mark means a screen that has just opened: give it the present, not a
    // backlog of everything since the host started.
    let mark = args.mark.unwrap_or_else(|| news.current_mark());
    encode(news.catch_up(&mark, ctx.actor))
}

pub fn register(registry: &mut Registry) {
    // Everyone signed in may ask; what they hear is filtered by their grants
    // inside the handler.
    registry.register("news_catch_up", Required::signed_in(), h_catch_up);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::Grants;

    fn owner() -> Actor {
        Actor::new("u-sam".into(), "Sam".into(), true, Grants::all())
    }

    fn money_only() -> Actor {
        Actor::new(
            "u-alex".into(),
            "Alex".into(),
            false,
            Grants::none().with(Area::Money, Access::Read),
        )
    }

    fn changed(area: Area, id: &str) -> Notice {
        Notice::RecordChanged {
            area,
            record_kind: "budget".into(),
            record_id: id.into(),
        }
    }

    fn notices(result: &CatchUp) -> &[Notice] {
        match result {
            CatchUp::Notices { notices, .. } => notices,
            CatchUp::StartOver { .. } => panic!("expected notices, got start_over"),
        }
    }

    fn mark_of(result: &CatchUp) -> &Mark {
        match result {
            CatchUp::Notices { mark, .. } | CatchUp::StartOver { mark } => mark,
        }
    }

    #[test]
    fn a_fresh_client_hears_nothing_and_starts_from_now() {
        let news = News::new();
        news.publish(changed(Area::Planning, "b1"));

        // The mark taken after that event does not replay it.
        let mark = news.current_mark();
        let result = news.catch_up(&mark, &owner());
        assert!(notices(&result).is_empty());
    }

    #[test]
    fn events_after_the_mark_are_delivered_in_order() {
        let news = News::new();
        let mark = news.current_mark();

        news.publish(changed(Area::Planning, "b1"));
        news.publish(changed(Area::Planning, "b2"));

        let result = news.catch_up(&mark, &owner());
        let heard = notices(&result);
        assert_eq!(heard.len(), 2);
        assert_eq!(heard[0], changed(Area::Planning, "b1"));
        assert_eq!(heard[1], changed(Area::Planning, "b2"));
    }

    #[test]
    fn the_returned_mark_does_not_replay_what_was_just_heard() {
        let news = News::new();
        let mut mark = news.current_mark();

        news.publish(changed(Area::Planning, "b1"));
        let first = news.catch_up(&mark, &owner());
        assert_eq!(notices(&first).len(), 1);

        mark = mark_of(&first).clone();
        let second = news.catch_up(&mark, &owner());
        assert!(
            notices(&second).is_empty(),
            "the same event was delivered twice"
        );
    }

    #[test]
    fn news_carries_no_row_data_only_what_to_re_read() {
        // The notice names the record; it does not carry its contents. If this
        // ever gains an `amount` or a `name`, news has become a second read
        // path and the permission checks can drift.
        let encoded = serde_json::to_value(changed(Area::Planning, "b1")).unwrap();
        let fields: Vec<String> = encoded
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            fields,
            vec![
                "area".to_string(),
                "kind".to_string(),
                "record_id".to_string(),
                "record_kind".to_string()
            ]
        );
    }

    // ------------------------------------------------------ area scoping

    #[test]
    fn a_person_hears_only_areas_they_may_read() {
        let news = News::new();
        let mark = news.current_mark();

        news.publish(changed(Area::Money, "a1"));
        news.publish(changed(Area::Admin, "u1"));

        let heard = news.catch_up(&mark, &money_only());
        let heard = notices(&heard);
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0], changed(Area::Money, "a1"));
    }

    /// The subtle half of area scoping: filtered events must still move the
    /// mark, or someone without the grant asks about them forever.
    #[test]
    fn the_mark_advances_past_events_a_person_cannot_hear() {
        let news = News::new();
        let mark = news.current_mark();

        news.publish(changed(Area::Admin, "u1"));
        news.publish(changed(Area::Admin, "u2"));

        let result = news.catch_up(&mark, &money_only());
        assert!(notices(&result).is_empty(), "Alex should hear nothing");

        let advanced = mark_of(&result);
        assert!(
            advanced.seq > mark.seq,
            "the mark stalled at {} despite two events passing",
            mark.seq
        );

        // And asking again from the advanced mark still hears nothing new,
        // rather than re-examining the same two.
        let again = news.catch_up(advanced, &money_only());
        assert!(notices(&again).is_empty());
    }

    #[test]
    fn everyone_hears_maintenance_regardless_of_grants() {
        let news = News::new();
        let mark = news.current_mark();

        news.publish(Notice::MaintenanceOn {
            closed_by: "Sam".into(),
        });

        let nobody = Actor::new("u0".into(), "Nobody".into(), false, Grants::none());
        let heard = news.catch_up(&mark, &nobody);
        assert_eq!(
            notices(&heard).len(),
            1,
            "someone who cannot hear the closed sign would not know why saves fail"
        );
    }

    // ------------------------------------------------- falling behind

    #[test]
    fn a_client_too_far_behind_is_told_to_start_over() {
        let news = News::new();
        let stale = news.current_mark();

        for i in 0..(NEWS_CAPACITY + 10) {
            news.publish(changed(Area::Planning, &format!("b{i}")));
        }

        let result = news.catch_up(&stale, &owner());
        assert!(
            matches!(result, CatchUp::StartOver { .. }),
            "a client past the end of the ring must not be handed a partial picture"
        );
    }

    #[test]
    fn the_ring_is_capped() {
        let news = News::new();
        for i in 0..(NEWS_CAPACITY * 2) {
            news.publish(changed(Area::Planning, &format!("b{i}")));
        }
        assert_eq!(news.held(), NEWS_CAPACITY);
    }

    #[test]
    fn a_client_at_the_very_edge_of_the_ring_is_still_served() {
        let news = News::new();
        let mark = news.current_mark();

        // Exactly fills the ring; nothing has been dropped yet.
        for i in 0..NEWS_CAPACITY {
            news.publish(changed(Area::Planning, &format!("b{i}")));
        }

        let result = news.catch_up(&mark, &owner());
        assert_eq!(
            notices(&result).len(),
            NEWS_CAPACITY,
            "a client that missed nothing should not be told to start over"
        );
    }

    #[test]
    fn start_over_hands_back_a_usable_mark() {
        let news = News::new();
        let stale = news.current_mark();
        for i in 0..(NEWS_CAPACITY + 10) {
            news.publish(changed(Area::Planning, &format!("b{i}")));
        }

        let result = news.catch_up(&stale, &owner());
        let recovered = mark_of(&result).clone();

        news.publish(changed(Area::Planning, "after"));
        let next = news.catch_up(&recovered, &owner());
        assert_eq!(notices(&next).len(), 1, "the recovered mark should work");
    }

    /// A mark from a previous run reads identically to being too far behind,
    /// which is what makes re-sync after a host restart free.
    #[test]
    fn a_mark_from_a_previous_run_is_told_to_start_over() {
        let before_restart = News::new();
        let mark = before_restart.current_mark();

        let after_restart = News::new(); // as if the host was restarted
        after_restart.publish(changed(Area::Planning, "b1"));

        let result = after_restart.catch_up(&mark, &owner());
        assert!(
            matches!(result, CatchUp::StartOver { .. }),
            "a mark from the previous run must not be read against the new run's sequence"
        );
    }

    #[test]
    fn a_mark_from_a_previous_run_is_rejected_even_when_the_sequence_looks_valid() {
        // The sequence number alone would look perfectly reasonable here — it
        // is the run that gives it away.
        let after_restart = News::new();
        after_restart.publish(changed(Area::Planning, "b1"));

        let plausible_but_stale = Mark {
            run: Uuid::new_v4().to_string(),
            seq: 0,
        };
        let result = after_restart.catch_up(&plausible_but_stale, &owner());
        assert!(matches!(result, CatchUp::StartOver { .. }));
    }

    #[test]
    fn recovering_from_a_restart_needs_no_special_case_at_the_caller() {
        // Same code path as any other start_over: take the mark and carry on.
        let news = News::new();
        let stale = Mark {
            run: "some-earlier-run".into(),
            seq: 42,
        };

        let result = news.catch_up(&stale, &owner());
        let recovered = mark_of(&result).clone();
        assert_eq!(recovered.run, news.run);

        news.publish(changed(Area::Planning, "b1"));
        let next = news.catch_up(&recovered, &owner());
        assert_eq!(notices(&next).len(), 1);
    }

    // --------------------------------------------------------- ordering

    #[test]
    fn busy_and_freed_carry_the_holders_name_and_the_record() {
        let news = News::new();
        let mark = news.current_mark();

        news.publish(Notice::RecordBusy {
            area: Area::Planning,
            record_kind: "budget".into(),
            record_id: "groceries".into(),
            holder: "Sam".into(),
        });
        news.publish(Notice::RecordFreed {
            area: Area::Planning,
            record_kind: "budget".into(),
            record_id: "groceries".into(),
        });

        let result = news.catch_up(&mark, &owner());
        let heard = notices(&result);
        assert_eq!(heard.len(), 2);

        match &heard[0] {
            Notice::RecordBusy { holder, record_id, .. } => {
                assert_eq!(holder, "Sam");
                assert_eq!(record_id, "groceries");
            }
            other => panic!("expected RecordBusy, got {other:?}"),
        }
        assert!(matches!(heard[1], Notice::RecordFreed { .. }));
    }

    #[test]
    fn notices_round_trip_through_json() {
        let all = vec![
            changed(Area::Money, "a1"),
            Notice::RecordBusy {
                area: Area::Planning,
                record_kind: "budget".into(),
                record_id: "b1".into(),
                holder: "Sam".into(),
            },
            Notice::RecordFreed {
                area: Area::Planning,
                record_kind: "budget".into(),
                record_id: "b1".into(),
            },
            Notice::MaintenanceOn {
                closed_by: "Sam".into(),
            },
            Notice::MaintenanceOff,
        ];

        for notice in all {
            let encoded = serde_json::to_string(&notice).unwrap();
            let decoded: Notice = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, notice);
        }
    }

    #[test]
    fn a_catch_up_result_round_trips_through_json() {
        let news = News::new();
        let mark = news.current_mark();
        news.publish(changed(Area::Planning, "b1"));

        let result = news.catch_up(&mark, &owner());
        let encoded = serde_json::to_string(&result).unwrap();
        let decoded: CatchUp = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, result);
    }
}
