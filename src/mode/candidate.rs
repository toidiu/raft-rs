use crate::{
    io::{ServerIO, IO_BUF_LEN},
    log::TermIdx,
    mode::{Context, Mode, ModeTransition},
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
    pub fn on_candidate<IO: ServerIO>(
        &mut self,
        tx: &mut IO,
        context: &mut Context<IO>,
    ) -> ModeTransition {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(tx, context)
    }

    pub fn on_timeout<IO: ServerIO>(
        &mut self,
        tx: &mut IO,
        context: &mut Context<IO>,
    ) -> ModeTransition {
        //% Compliance:
        //% If election timeout elapses: start new election
        self.start_election(tx, context)
    }

    pub fn on_recv<IO: ServerIO>(
        &mut self,
        tx: &mut IO,
        rpc: crate::rpc::Rpc,
        context: &mut Context<IO>,
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

    fn start_election<IO: ServerIO>(
        &mut self,
        _tx: &mut IO,
        context: &mut Context<IO>,
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
        for (_id, peer) in context.peer_map.iter_mut() {
            let prev_log_term_idx = TermIdx::prev_term_idx(peer, context.state);
            Rpc::new_request_vote(current_term, context.server_id, prev_log_term_idx)
                .encode_mut(&mut buf);
            peer.send(buf.as_mut_slice().to_vec());
        }

        ModeTransition::None
    }

    fn cast_vote<IO: ServerIO>(
        &mut self,
        vote_for_server: ServerId,
        context: &mut Context<IO>,
    ) -> ElectionResult {
        let self_id = context.server_id;
        if self_id == vote_for_server {
            // voting for self
            context.state.voted_for = Some(self_id);
            self.on_vote_received(self_id, context)
        } else {
            debug_assert!(
                context.peer_map.contains_key(&vote_for_server),
                "voted for invalid server id"
            );
            context.state.voted_for = Some(vote_for_server);
            ElectionResult::Pending
        }
    }

    fn on_vote_received<IO: ServerIO>(
        &mut self,
        id: ServerId,
        context: &Context<IO>,
    ) -> ElectionResult {
        debug_assert!(
            context.peer_map.contains_key(&id) || id == context.server_id,
            "voted for invalid server id"
        );
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
    use super::*;
    use crate::{
        io::testing::MockIO, peer::Peer, server::ServerId, state::State, timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::DecoderBuffer;

    #[tokio::test]
    async fn test_start_election() {
        let mut io = MockIO::new();
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer_fill = 6;
        let server_id = ServerId::new([peer_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer_fill]);
        // let server_id = ServerId::new([6; 16]);
        // let mut peer_map = vec![Peer::new(ServerId::new([1; 16]))];
        let mut state = State::new(timeout, &peer_map);
        let mut context = Context {
            server_id,
            state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut candidate = CandidateState::default();

        let transition = candidate.start_election(&mut io, &mut context);
        assert!(matches!(transition, ModeTransition::None));

        // construct RPC to compare
        let mut current_term = state.current_term;
        current_term.increment();
        let expected_rpc = Rpc::new_request_vote(current_term, server_id, TermIdx::initial());

        // FIXME revive test
        // let rpc_bytes = io.send_queue.pop().unwrap();
        // let buffer = DecoderBuffer::new(&rpc_bytes);
        // let (sent_request_vote, _) = buffer.decode::<Rpc>().unwrap();
        // assert_eq!(expected_rpc, sent_request_vote);
    }

    #[tokio::test]
    async fn test_start_election_with_no_peers() {
        let mut io = MockIO::new();
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());
        let server_id = ServerId::new([6; 16]);

        let mut peer_map = Peer::mock_as_map(&[]);
        let mut state = State::new(timeout, &peer_map);
        let mut context = Context {
            server_id,
            state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut candidate = CandidateState::default();
        assert_eq!(Mode::quorum(&context), 1);

        // Elect self
        let transition = candidate.start_election(&mut io, &mut context);
        assert!(matches!(transition, ModeTransition::ToLeader));

        // No RPC sent
        assert!(io.send_queue.is_empty());
    }

    #[tokio::test]
    async fn test_cast_vote() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let self_id = ServerId::new([1; 16]);
        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer2_fill, 3]);
        let mut state = State::new(timeout, &peer_map);

        let context = Context {
            server_id: self_id,
            state: &mut state,
            peer_map: &mut peer_map,
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
