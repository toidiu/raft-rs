use crate::{
    io::ServerIO,
    log::{Idx, Log, Term, TermIdx},
    peer::Peer,
    server::ServerId,
    timeout::Timeout,
};
use std::collections::BTreeMap;

pub struct State {
    //  ==== Persistent state on all servers ====
    //% Compliance:
    //% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases
    //% monotonically)
    pub current_term: Term,

    //% Compliance:
    //% `votedFor` `candidateId` that received vote in current term (or null if none)
    pub voted_for: Option<ServerId>,

    //% Compliance:
    //% `log[]` log entries; each entry contains command for state machine, and term when entry was
    //% received by leader (first index is 1)
    pub log: Log,

    // ==== Volatile state on all servers ====
    //% Compliance:
    //% `commitIndex` index of highest log entry known to be committed (initialized to 0, increases
    //% monotonically)
    pub commit_idx: Idx,

    //% Compliance:
    //% lastApplied: index of highest log entry applied to state machine (initialized to 0,
    //% increases monotonically)
    pub last_applied: Idx,

    // ==== Volatile state on leaders ====
    //% Compliance:
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    pub next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance:
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    pub match_idx: BTreeMap<ServerId, Idx>,

    pub election_timer: Timeout,
}

impl State {
    pub fn new<T: ServerIO>(
        election_timer: Timeout,
        peer_map: &BTreeMap<ServerId, Peer<T>>,
    ) -> Self {
        let log = Log::new();
        let mut next_idx_map = BTreeMap::new();
        let mut match_idx_map = BTreeMap::new();

        //% Compliance:
        //% `nextIndex[]` for each server, index of the next log entry to send to that server
        //% (initialized to leader last log index + 1)
        let next_log_idx = log.next_idx();

        //% Compliance:
        //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
        //% (initialized to 0, increases monotonically)
        let match_idx = Idx::initial();

        for (id, peer) in peer_map.iter() {
            let Peer { id: _, io: _ } = peer;

            next_idx_map.insert(*id, next_log_idx);
            match_idx_map.insert(*id, match_idx);
        }
        State {
            current_term: Term::initial(),
            voted_for: None,
            log,
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
            next_idx: next_idx_map,
            match_idx: match_idx_map,
            election_timer,
        }
    }

    // Calculate the prev TermIdx for the peer
    pub fn peers_prev_term_idx<T: ServerIO>(&self, peer: &Peer<T>) -> TermIdx {
        // next Idx should always be > 0
        let peers_next_idx = self.next_idx.get(&peer.id).unwrap();
        assert!(!peers_next_idx.is_initial());

        if *peers_next_idx == Idx::from(1) {
            // peer's log is empty so its prev_term_idx value is inital value
            TermIdx::initial()
        } else {
            let prev_idx = *peers_next_idx - 1;
            let term_at_prev_idx = self.log.term_at_idx(&prev_idx).expect(
                "next_idx_map incorrectly indicates that an Entry exists at idx: {prev_idx}",
            );
            TermIdx::builder()
                .with_term(term_at_prev_idx)
                .with_idx(prev_idx)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        log::Entry,
        state::{Idx, Peer, ServerId, State, Term, TermIdx},
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn peers_prev_term_idx_for_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let peer_map = Peer::mock_as_map(&[peer2_fill, 3]);
        let state = State::new(timeout, &peer_map);

        // retrieve the prev_term_idx for peer (expect initial since log is empty)
        let peer2 = peer_map.get(&peer2_id).unwrap();
        let term_idx = state.peers_prev_term_idx(peer2);
        assert!(term_idx.is_initial());
    }

    #[tokio::test]
    async fn peers_prev_term_idx_for_non_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let peer_map = Peer::mock_as_map(&[peer2_fill, 3]);
        let mut state = State::new(timeout, &peer_map);

        // insert a log entry
        let next_idx = Idx::from(2);
        let expected_term = Term::from(4);
        let entry = Entry {
            term: expected_term,
            command: 8,
        };

        // update state so it contains some log entries
        state.log.push(vec![entry]);
        state.next_idx.insert(peer2_id, next_idx);

        // retrieve the prev_term_idx for peer
        let peer2 = peer_map.get(&peer2_id).unwrap();
        let term_idx = state.peers_prev_term_idx(peer2);
        assert_eq!(
            term_idx,
            TermIdx::builder()
                .with_term(expected_term)
                .with_idx(next_idx - 1)
        );
    }
}
