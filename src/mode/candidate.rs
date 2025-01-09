use crate::{
    io::IO_BUF_LEN,
    log::TermIdx,
    mode::{Context, Mode, ModeTransition, ServerTx},
    rpc::Rpc,
    server::ServerId,
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct CandidateState {
    votes_received: HashSet<ServerId>,
}

impl CandidateState {
    pub fn on_candidate<T: ServerTx>(
        &mut self,
        tx: &mut T,
        context: &mut Context,
    ) -> ModeTransition {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(tx, context)
    }

    pub fn on_timeout<T: crate::io::ServerTx>(
        &mut self,
        tx: &mut T,
        context: &mut Context,
    ) -> ModeTransition {
        //% Compliance:
        //% If election timeout elapses: start new election
        self.start_election(tx, context)
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        tx: &mut T,
        rpc: crate::rpc::Rpc,
        context: &mut Context,
    ) -> (ModeTransition, Option<Rpc>) {
        match rpc {
            Rpc::RequestVote(_request_vote_state) => todo!(),
            Rpc::RespRequestVote(_resp_request_vote_state) => todo!(),
            Rpc::AppendEntries(ref append_entries_state) => {
                //% Compliance:
                //% If AppendEntries RPC received from new leader: convert to follower

                //% Compliance:
                //% a candidate receives AppendEntries from another server claiming to be a leader
                if append_entries_state.term >= context.state.current_term {
                    //% Compliance:
                    //% if that leader's current term is >= the candidate's
                    //
                    //% Compliance:
                    //% then the candidate reverts to a follower
                    return (ModeTransition::ToFollower, Some(rpc));
                } else {
                    //% Compliance:
                    //% if the leader's current term is < the candidate's
                    //
                    //% Compliance:
                    //% reject the RPC and continue in the candidate state
                    let mut slice = vec![0; IO_BUF_LEN];
                    let mut buf = EncoderBuffer::new(&mut slice);
                    let term = context.state.current_term;
                    Rpc::new_append_entry_resp(term, false).encode_mut(&mut buf);
                    tx.send(buf.as_mut_slice().to_vec());
                }
            }
            Rpc::RespAppendEntries(_resp_append_entries_state) => (),
        }

        (ModeTransition::None, None)
    }

    fn start_election<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        context: &mut Context,
    ) -> ModeTransition {
        //% Compliance:
        //% Increment currentTerm
        context.state.current_term.increment();

        //% Compliance:
        //% Vote for self
        if matches!(
            self.cast_vote(context.server_id, context),
            ElectionResult::Elected
        ) {
            //% Compliance:
            //% If votes received from majority of servers: become leader
            return ModeTransition::ToLeader;
        }

        //% Compliance:
        //% Reset election timer
        context.state.election_timer.reset();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        let current_term = context.state.current_term;
        //% Compliance:
        //% Send RequestVote RPCs to all other servers
        for (_id, peer) in context.peer_list.iter_mut() {
            let prev_log_term_idx = TermIdx::prev_term_idx(peer, context.state);
            Rpc::new_request_vote(current_term, context.server_id, prev_log_term_idx)
                .encode_mut(&mut buf);
            peer.send(buf.as_mut_slice().to_vec());
        }
        // FIXME send a RequestVote to all peers
        // let mut slice = vec![0; IO_BUF_LEN];
        // let mut buf = EncoderBuffer::new(&mut slice);
        // let term = context.state.current_term;
        //
        // FIXME cleanup
        // let next_log_idx = context.state.next_idx.get(&ServerId::new([0; 16])).unwrap();
        // let prev_log_idx = Idx::from(next_log_idx.0 - 1);
        // let prev_log_term = context.log.last_term();
        // let prev_log_term_idx = TermIdx::builder().with_term(prev_log_term).with_idx(prev_log_idx);
        // Rpc::new_request_vote(term, context.server_id, prev_log_term_idx).encode_mut(&mut buf);
        // tx.send(buf.as_mut_slice().to_vec());

        ModeTransition::None
    }

    fn cast_vote(&mut self, vote_for_server: ServerId, context: &mut Context) -> ElectionResult {
        let self_id = context.server_id;
        if self_id == vote_for_server {
            // voting for self
            context.state.voted_for = Some(self_id);
            self.on_vote_received(self_id, context)
        } else {
            // debug_assert!(
            //     context.peer_list.contains(&Peer::new(vote_for_server)),
            //     "voted for invalid server id"
            // );
            context.state.voted_for = Some(vote_for_server);
            ElectionResult::Pending
        }
    }

    fn on_vote_received(&mut self, id: ServerId, context: &Context) -> ElectionResult {
        // debug_assert!(
        //     context.peer_list.contains(&Peer::new(id)) || id == context.server_id,
        //     "voted for invalid server id"
        // );
        self.votes_received.insert(id);

        if self.votes_received.len() >= Mode::quorum(context) {
            ElectionResult::Elected
        } else {
            ElectionResult::Pending
        }
    }
}

#[must_use]
pub enum ElectionResult {
    Elected,
    Pending,
}

#[cfg(test)]
mod tests {

    use crate::peer::Peer;
use super::*;
    use crate::{io::testing::MockTx, server::ServerId, state::State, timeout::Timeout};
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn test_start_election() {
        let mut tx = MockTx::new();
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer_fill = 6;
        let server_id = ServerId::new([peer_fill; 16]);
        let mut peer_list = Peer::mock_as_map(&[peer_fill]);
        // let server_id = ServerId::new([6; 16]);
        // let mut peer_list = vec![Peer::new(ServerId::new([1; 16]))];
        let mut state = State::new(timeout, &peer_list);
        let mut context = Context {
            server_id,
            state: &mut state,
            peer_list: &mut peer_list,
        };
        let mut candidate = CandidateState::default();

        let transition = candidate.start_election(&mut tx, &mut context);
        assert!(matches!(transition, ModeTransition::None));

        // construct RPC to compare
        let mut current_term = state.current_term;
        current_term.increment();
        let _expected_rpc = Rpc::new_request_vote(current_term, server_id, TermIdx::initial());

        // let rpc_bytes = tx.queue.pop().unwrap();
        // let buffer = DecoderBuffer::new(&rpc_bytes);
        // let (sent_request_vote, _) = buffer.decode::<Rpc>().unwrap();
        // assert_eq!(expected_rpc, sent_request_vote);
    }

    #[tokio::test]
    async fn test_start_election_with_no_peers() {
        let mut tx = MockTx::new();
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());
        let server_id = ServerId::new([6; 16]);

        let mut peer_list = Peer::mock_as_map(&[]);
        let mut state = State::new(timeout, &peer_list);
        let mut context = Context {
            server_id,
            state: &mut state,
            peer_list: &mut peer_list,
        };
        let mut candidate = CandidateState::default();
        assert_eq!(Mode::quorum(&context), 1);

        // Elect self
        let transition = candidate.start_election(&mut tx, &mut context);
        assert!(matches!(transition, ModeTransition::ToLeader));

        // No RPC sent
        assert!(tx.queue.is_empty());
    }

    #[tokio::test]
    async fn test_cast_vote() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let self_id = ServerId::new([1; 16]);
        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let mut peer_list = Peer::mock_as_map(&[peer2_fill, 3]);
        let mut state = State::new(timeout, &peer_list);

        let context = Context {
            server_id: self_id,
            state: &mut state,
            peer_list: &mut peer_list
        };
        let mut candidate = CandidateState::default();
        assert_eq!(Mode::quorum(&context), 2);
        assert!(Mode::quorum(&context) > candidate.votes_received.len());

        // Receive peer's vote
        assert!(matches!(
            candidate.on_vote_received(peer2_id, &context),
            ElectionResult::Pending
        ));
        assert!(Mode::quorum(&context) > candidate.votes_received.len());

        // Don't count same vote
        assert!(matches!(
            candidate.on_vote_received(peer2_id, &context),
            ElectionResult::Pending
        ));
        assert!(Mode::quorum(&context) > candidate.votes_received.len());

        // Vote for self and reach quorum
        assert!(matches!(
            candidate.on_vote_received(self_id, &context),
            ElectionResult::Elected
        ));
        assert_eq!(Mode::quorum(&context), 2);
    }
}
