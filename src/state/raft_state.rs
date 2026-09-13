use crate::{
    server::{Id, PeerId, ServerId},
    state::{
        log::{Idx, Log, Term, TermIdx},
        state_machine::{CommitEntry, CurrentMode, StateMachine},
    },
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
    //% lastApplied: index of highest log entry 'committed'/'applied' to state machine (initialized
    //% to 0, % increases monotonically)
    last_applied: Idx,

    // The permanent storage which stored Entries replicated on majority of Raft servers.
    state_machine: StateMachine,

    pub election_timer: Timeout,
}

impl RaftState {
    pub fn new(election_timer: Timeout) -> Self {
        let log = Log::new();
        let state_machine = StateMachine::new();

        RaftState {
            current_term: Term::initial(),
            voted_for: None,
            log,
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
            election_timer,
            state_machine,
        }
    }

    pub fn last_applied(&self) -> &Idx {
        &self.last_applied
    }

    pub fn commit_idx(&self) -> &Idx {
        &self.commit_idx
    }

    /// The entries that have been committed to the StateMachine.
    pub fn applied_entries(&self) -> Vec<Idx> {
        self.state_machine.applied_entries()
    }

    /// Commit Entries in the StateMachine up to the commit_up_to_idx. Use `last_log_term_idx` to
    /// figure out the last idx that was committed.
    pub fn update_commit_idx(
        &mut self,
        commit_up_to_idx: Idx,
        updated_peers: &[PeerId],
        mode: CurrentMode,
    ) {
        assert!(
            commit_up_to_idx >= self.commit_idx,
            "commitIdx is monotonically increasing"
        );
        if commit_up_to_idx > self.commit_idx {
            // TODO: (replace with metrics)
            // Detect if we are actually every sending more than 1 Entry.
            // Sending 1 Entry per RPC was meant to be a simplification for the POC.
            assert!(
                commit_up_to_idx == self.commit_idx + 1,
                "Comitting more than 1 entry!! its not wrong but understand this path."
            );
        }

        // Commit the entries to the StateMachine.
        while &commit_up_to_idx > self.last_applied() {
            //% Compliance:
            //% If commitIndex > lastApplied: increment lastApplied
            self.last_applied += 1;

            //% Compliance:
            //% If commitIndex > lastApplied: apply log[lastApplied] to state machine (§5.3)
            let entry_at_last_applied = self
                .log
                .find_entry_at(self.last_applied())
                .expect("should have an entry at index lastApplied");
            let commit_entry = CommitEntry {
                entry: entry_at_last_applied.clone(),
                log_last_applied_idx: *self.last_applied(),
                updated_peers: updated_peers.to_vec(),
                mode,
            };

            // Commit the entry and update commit_idx.
            self.state_machine.commit_entry(commit_entry);
            self.commit_idx = *self.last_applied();
        }
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
        state::{
            entry::Entry,
            raft_state::{Idx, RaftState, Term, TermIdx},
        },
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
        state.log.test_append_entries(vec![Entry::new(t1, 8)]);
        state.log.test_append_entries(vec![Entry::new(t1, 8)]);
        // on_start_election should increment the currentTerm and return the last log TermIdx
        let last_log_term_idx = state.on_start_election();
        assert_eq!(
            last_log_term_idx,
            TermIdx::builder().with_term(t1).with_idx(Idx::from(2))
        );
        assert_eq!(state.current_term, current_term + 1);

        // Insert 2 entries for Term 2
        let t2 = Term::from(2);
        state.log.test_append_entries(vec![Entry::new(t2, 8)]);
        state.log.test_append_entries(vec![Entry::new(t2, 8)]);
        // on_start_election should increment the currentTerm and return the last log TermIdx
        let last_log_term_idx = state.on_start_election();
        assert_eq!(
            last_log_term_idx,
            TermIdx::builder().with_term(t2).with_idx(Idx::from(4))
        );
        assert_eq!(state.current_term, current_term + 2);
    }
}
