use crate::{
    clock::{Clock, Timer},
    log::{Log, TermIdx},
    state::{ServerId, Term},
};
use std::collections::HashSet;

#[must_use]
pub enum ElectionResult {
    Elected,
    Pending,
}

#[derive(Debug)]
pub struct Inner {
    pub id: ServerId,

    // # Compliance: Fig 2
    // currentTerm: latest term server has seen (initialized to 0 on first boot, increases
    // monotonically)
    pub curr_term: Term,

    // list of all raft servers
    server_list: Vec<ServerId>,

    // # Compliance: Fig 2
    // votedFor: candidateId that received vote in current
    voted_for: Option<ServerId>,
    pub votes_received: HashSet<ServerId>,

    // # Compliance: Fig 2
    // log[]: log entries; each entry contains command for state machine, and term when entry was
    // received by leader (first index is 1)
    log: Log,

    // TODO
    // lastApplied: index of highest log entry applied to state machine (initialized to 0, increases
    // monotonically)
    // last_applied: todo!(),
    pub timer: Timer,
}

impl Inner {
    pub fn new(clock: Clock, server_list: Vec<ServerId>) -> Self {
        Inner {
            id: ServerId::new(),
            curr_term: Term(0),
            server_list,
            voted_for: None,
            votes_received: HashSet::new(),
            log: Log::new(),
            timer: Timer::new(clock),
        }
    }

    pub fn on_new_election(&mut self) {
        self.voted_for = None;
        self.votes_received.clear();
    }

    pub fn on_vote_received(&mut self, id: ServerId) -> ElectionResult {
        debug_assert!(self.server_list.contains(&id) || id == self.id);
        self.votes_received.insert(id);

        if self.votes_received.len() >= self.quorum() {
            // # Compliance:
            // If votes received from majority of servers: become leader
            ElectionResult::Elected
        } else {
            ElectionResult::Pending
        }
    }

    pub fn cast_vote(&mut self, id: ServerId) -> ElectionResult {
        debug_assert!(self.voted_for.is_none());

        if self.id == id {
            self.voted_for = Some(self.id);
            self.on_vote_received(self.id)
        } else {
            debug_assert!(self.server_list.contains(&id));
            self.voted_for = Some(id);
            ElectionResult::Pending
        }
    }

    pub fn voted_for(&self) -> Option<ServerId> {
        self.voted_for
    }

    // # Compliance: Fig 2
    // commitIndex: index of highest log entry known to be committed (initialized to 0, increases
    // monotonically)
    pub fn last_committed_term_idx(&self) -> TermIdx {
        self.log.last_committed_term_idx()
    }

    fn is_elected(&self) -> bool {
        self.votes_received.len() >= self.quorum()
    }

    fn quorum(&self) -> usize {
        let peer_plus_self = self.server_list.len() + 1;
        let half = peer_plus_self / 2;
        half + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quorum() {
        let inner = Inner::new(Clock::default(), vec![]);
        assert_eq!(inner.quorum(), 1);

        let inner = Inner::new(Clock::default(), vec![ServerId::new()]);
        assert_eq!(inner.quorum(), 2);

        let inner = Inner::new(Clock::default(), vec![ServerId::new(), ServerId::new()]);
        assert_eq!(inner.quorum(), 2);

        let inner = Inner::new(
            Clock::default(),
            vec![ServerId::new(), ServerId::new(), ServerId::new()],
        );
        assert_eq!(inner.quorum(), 3);
    }

    #[tokio::test]
    async fn test_cast_vote() {
        let server_list = vec![ServerId::new(), ServerId::new()];
        let mut inner = Inner::new(Clock::default(), server_list.clone());

        assert!(matches!(
            inner.on_vote_received(inner.id),
            ElectionResult::Pending
        ));
        assert_eq!(inner.votes_received.len(), 1);

        inner.on_new_election();
        assert_eq!(inner.votes_received.len(), 0);

        assert!(matches!(
            inner.on_vote_received(inner.id),
            ElectionResult::Pending
        ));
        assert!(matches!(
            inner.on_vote_received(server_list[0]),
            ElectionResult::Elected
        ));
        assert_eq!(inner.votes_received.len(), 2);
    }
}
