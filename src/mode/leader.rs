use crate::{
    io::{ServerIO, IO_BUF_LEN},
    log::{Idx, TermIdx},
    mode::Context,
    peer::Peer,
    rpc::Rpc,
    server::ServerId,
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Leader {
    // ==== Volatile state on leaders ====
    //% Compliance:
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    pub next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance:
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    pub match_idx: BTreeMap<ServerId, Idx>,
}

impl Leader {
    pub fn new<IO: ServerIO>(context: &mut Context<IO>) -> Self {
        let mut next_idx_map = BTreeMap::new();
        let mut match_idx_map = BTreeMap::new();

        //% Compliance:
        //% `nextIndex[]` for each server, index of the next log entry to send to that server
        //% (initialized to leader last log index + 1)
        let next_log_idx = context.state.log.next_idx();

        //% Compliance:
        //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
        //% (initialized to 0, increases monotonically)
        let match_idx = Idx::initial();

        for (id, peer) in context.peer_map.iter() {
            let Peer { id: _, io: _ } = peer;

            next_idx_map.insert(*id, next_log_idx);
            match_idx_map.insert(*id, match_idx);
        }

        Leader {
            next_idx: next_idx_map,
            match_idx: match_idx_map,
        }
    }

    pub fn on_leader<IO: ServerIO>(&mut self, context: &mut Context<IO>) {
        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        self.send_heartbeat(context);
    }

    pub fn send_heartbeat<IO: ServerIO>(&mut self, context: &mut Context<IO>) {
        let current_term = context.state.current_term;
        let self_id = context.server_id;
        let commit_idx = context.state.commit_idx;

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        for (_id, peer) in context.peer_map.iter_mut() {
            //% Compliance:
            //% prevLogIndex: index of log entry immediately preceding new ones
            //% prevLogTerm: term of prevLogIndex entry
            let prev_log_term_idx = context.state.log.last_term_idx();
            Rpc::new_append_entry(current_term, self_id, prev_log_term_idx, commit_idx, vec![])
                .encode_mut(&mut buf);
            peer.send(buf.as_mut_slice().to_vec());
        }
    }

    pub fn on_timeout<IO: ServerIO>(&mut self, context: &mut Context<IO>) {
        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        self.send_heartbeat(context);
    }

    pub fn on_recv<IO: ServerIO>(&mut self, _rpc: crate::rpc::Rpc, _context: &mut Context<IO>) {
        todo!()
    }

    // Calculate the prev TermIdx for the peer
    fn peers_prev_term_idx<IO: ServerIO>(
        &self,
        peer_id: ServerId,
        context: &mut Context<IO>,
    ) -> TermIdx {
        let peer = context
            .peer_map
            .get(&peer_id)
            .expect("peer doesn't exist in peer map");
        // next Idx should always be > 0
        let peers_next_idx = self.next_idx.get(&peer.id).unwrap();
        assert!(!peers_next_idx.is_initial());

        if *peers_next_idx == Idx::from(1) {
            // peer's log is empty so its prev_term_idx value is inital value
            TermIdx::initial()
        } else {
            let prev_idx = *peers_next_idx - 1;
            let term_at_prev_idx = context.state.log.term_at_idx(&prev_idx).expect(
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
    use super::*;
    use crate::{
        log::{Entry, Term},
        peer::Peer,
        server::{Context, ServerId},
        state::State,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn peers_prev_term_idx_for_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer2_fill, 3]);
        let mut state = State::new(timeout, &peer_map);
        let mut context = Context {
            server_id,
            state: &mut state,
            peer_map: &mut peer_map,
        };
        let leader = Leader::new(&mut context);

        // retrieve the prev_term_idx for peer (expect initial since log is empty)
        let term_idx = leader.peers_prev_term_idx(peer2_id, &mut context);
        assert!(term_idx.is_initial());
    }

    #[tokio::test]
    async fn peers_prev_term_idx_for_non_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer2_fill, 3]);
        let mut state = State::new(timeout, &peer_map);

        // insert a log entry
        let next_idx = Idx::from(2);
        let expected_term = Term::from(4);
        let entry = Entry {
            term: expected_term,
            command: 8,
        };

        let mut context = Context {
            server_id,
            state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut leader = Leader::new(&mut context);

        // update state so it contains some log entries
        context.state.log.push(vec![entry]);
        leader.next_idx.insert(peer2_id, next_idx);

        // retrieve the prev_term_idx for peer
        let term_idx = leader.peers_prev_term_idx(peer2_id, &mut context);
        assert_eq!(
            term_idx,
            TermIdx::builder()
                .with_term(expected_term)
                .with_idx(next_idx - 1)
        );
    }
}
