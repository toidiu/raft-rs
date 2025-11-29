use crate::{
    log::{Idx, Log, Term, TermIdx},
    server::{Id, PeerId, ServerId},
    timeout::Timeout,
};

pub struct RaftState {
    //  ==== Persistent state on all servers ====
    //% Compliance:
    //% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases
    //% monotonically)
    pub current_term: Term,

    //% Compliance:
    //% `votedFor` `candidateId` that received vote in current term (or null if none)
    voted_for: Option<Id>,

    //% Compliance:
    //% `log[]` log entries; each entry contains command for state machine, and term when entry was
    //% received by leader (first index is 1)
    pub log: Log,

    // ==== Volatile state on all servers ====
    //% Compliance:
    //% `commitIndex` index of highest log entry known to be committed (initialized to 0, increases
    //% monotonically)
    commit_idx: Idx,

    //% Compliance:
    //% lastApplied: index of highest log entry applied to state machine (initialized to 0,
    //% increases monotonically)
    pub last_applied: Idx,

    pub election_timer: Timeout,
}

impl RaftState {
    pub fn new(election_timer: Timeout) -> Self {
        let log = Log::new();

        RaftState {
            current_term: Term::initial(),
            voted_for: None,
            log,
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
            election_timer,
        }
    }

    pub fn commit_idx(&self) -> &Idx {
        &self.commit_idx
    }

    pub fn set_commit_idx(&mut self, idx: Idx) {
        self.commit_idx = idx;
    }

    // Retrieve the last TermIdx applied on self and increment the currentTerm
    pub fn on_start_election(&mut self) -> TermIdx {
        let last_log_term_idx = TermIdx::builder()
            .with_term(self.current_term)
            .with_idx(self.last_applied);

        //% Compliance:
        //% Increment currentTerm
        self.current_term.increment();

        last_log_term_idx
    }

    pub fn voted_for(&self) -> &Option<Id> {
        &self.voted_for
    }

    pub fn voted_for_self(&mut self, server_id: ServerId) {
        self.voted_for = Some(server_id.into_id())
    }

    pub fn voted_for_peer(&mut self, peer_id: PeerId) {
        self.voted_for = Some(peer_id.into_id())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        raft_state::{Idx, RaftState, Term, TermIdx},
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn on_start_election() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let mut state = RaftState::new(timeout);

        // set current term and idx
        let apply_term = Term::from(4);
        let apply_idx = Idx::from(2);
        state.current_term = apply_term;
        state.last_applied = apply_idx;
        assert_eq!(state.current_term, apply_term);
        assert_eq!(state.last_applied, apply_idx);

        // on_start_election should increment the currentTerm and return the previous TermIdx
        let prev_term_idx = state.on_start_election();

        assert_eq!(
            prev_term_idx,
            TermIdx::builder().with_term(apply_term).with_idx(apply_idx)
        );
        assert_eq!(state.current_term, Term::from(5));
    }
}
