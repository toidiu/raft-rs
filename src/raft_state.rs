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
    last_applied: Idx,

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

    pub fn last_applied(&self) -> &Idx {
        &self.last_applied
    }

    //% Compliance:
    //% lastApplied: index of highest log entry applied to state machine (initialized to 0,
    //% increases monotonically)
    //
    // TODO: apply to state machine
    pub fn increment_last_applied(&mut self) {
        self.last_applied += 1;
    }

    pub fn commit_idx(&self) -> &Idx {
        &self.commit_idx
    }

    pub fn set_commit_idx(&mut self, idx: Idx) {
        assert!(
            idx >= self.commit_idx,
            "commitIdx is monotonically increasing"
        );
        if idx > self.commit_idx {
            assert!(
                idx == self.commit_idx + 1,
                "we expect commitIdx should increase by 1 so each entry is captured in the log"
            );
        }
        self.commit_idx = idx;
    }

    // Retrieve the last log TermIdx and increment the currentTerm
    pub fn on_start_election(&mut self) -> TermIdx {
        //% Compliance:
        //% lastLogIndex: index of candidate’s last log entry (§5.4)
        //% lastLogTerm: term of candidate’s last log entry (§5.4)
        let last_log_term_idx = self.log.last_term_idx();

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
        log::Entry,
        raft_state::{Idx, RaftState, Term, TermIdx},
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn on_start_election() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        // Initialize state
        let mut state = RaftState::new(timeout);
        let current_term = Term::from(100);
        state.current_term = current_term;

        // Insert 2 entries for Term 1
        let t1 = Term::from(1);
        state.log.push(vec![Entry::new(t1, 8)]);
        state.log.push(vec![Entry::new(t1, 8)]);
        // on_start_election should increment the currentTerm and return the last log TermIdx
        let last_log_term_idx = state.on_start_election();
        assert_eq!(
            last_log_term_idx,
            TermIdx::builder().with_term(t1).with_idx(Idx::from(2))
        );
        assert_eq!(state.current_term, current_term + 1);

        // Insert 2 entries for Term 2
        let t2 = Term::from(2);
        state.log.push(vec![Entry::new(t2, 8)]);
        state.log.push(vec![Entry::new(t2, 8)]);
        // on_start_election should increment the currentTerm and return the last log TermIdx
        let last_log_term_idx = state.on_start_election();
        assert_eq!(
            last_log_term_idx,
            TermIdx::builder().with_term(t2).with_idx(Idx::from(4))
        );
        assert_eq!(state.current_term, current_term + 2);
    }
}
