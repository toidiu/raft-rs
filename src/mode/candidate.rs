use crate::{
    io::{ServerEgress, IO_BUF_LEN},
    mode::{Context, ElectionResult, Mode, ModeTransition},
    rpc::{AppendEntries, RequestVoteResp, Rpc},
    server::ServerId,
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct Candidate {
    votes_received: HashSet<ServerId>,
}

impl Candidate {
    pub fn on_candidate<E: ServerEgress>(&mut self, context: &mut Context<E>) -> ModeTransition {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(context)
    }

    pub fn on_timeout<E: ServerEgress>(&mut self, context: &mut Context<E>) -> ModeTransition {
        //% Compliance:
        //% A timeout occurs and there is no winner (can happen if too many servers become
        //% candidates at the same time)
        //% - increment its term
        //% - start a new election by initiating another round of RequestVote
        //
        //% Compliance:
        //% If election timeout elapses: start new election
        self.start_election(context)
    }

    pub fn on_recv<E: ServerEgress>(
        &mut self,
        peer_id: ServerId,
        rpc: crate::rpc::Rpc,
        context: &mut Context<E>,
    ) -> (ModeTransition, Option<Rpc>) {
        match rpc {
            Rpc::RequestVote(request_vote) => {
                request_vote.on_recv(context);
                (ModeTransition::None, None)
            }
            Rpc::RequestVoteResp(request_vote_resp) => {
                let transition =
                    self.on_recv_request_vote_resp(peer_id, request_vote_resp, context);
                (transition, None)
            }
            Rpc::AppendEntry(append_entries) => {
                self.on_recv_append_entries(append_entries, context)
            }
            Rpc::AppendEntryResp(_) => {
                todo!("it might be possible to get a response from a previous term")
            }
        }
    }

    fn on_recv_append_entries<E: ServerEgress>(
        &mut self,
        append_entries: AppendEntries,
        context: &mut Context<E>,
    ) -> (ModeTransition, Option<Rpc>) {
        let AppendEntries {
            term,
            leader_id,
            prev_log_term_idx: _,
            leader_commit_idx: _,
            entries: _,
        } = append_entries;
        let leader_io = &mut context.peer_map.get_mut(&leader_id).unwrap().io_egress;
        //% Compliance:
        //% another server establishes itself as a leader
        //% - a candidate receives AppendEntries from another server claiming to be a leader
        if term >= context.raft_state.current_term {
            //% Compliance:
            //% if that leader's current term is >= the candidate's
            //% - recognize the server as the new leader
            //% - then the candidate reverts to a follower
            //
            //% Compliance:
            //% If AppendEntries RPC received from new leader: convert to follower

            // Convert to Follower and process/respond to the RPC
            let rpc = Rpc::AppendEntry(append_entries);
            (ModeTransition::ToFollower, Some(rpc))
        } else {
            //% Compliance:
            //% if the leader's current term is < the candidate's
            //% - reject the RPC and continue in the candidate state
            let mut slice = vec![0; IO_BUF_LEN];
            let mut buf = EncoderBuffer::new(&mut slice);
            let term = context.raft_state.current_term;
            Rpc::new_append_entry_resp(term, false).encode_mut(&mut buf);
            leader_io.send(buf.as_mut_slice().to_vec());
            (ModeTransition::None, None)
        }
    }

    fn on_recv_request_vote_resp<E: ServerEgress>(
        &mut self,
        peer_id: ServerId,
        request_vote_resp: RequestVoteResp,
        context: &mut Context<E>,
    ) -> ModeTransition {
        let RequestVoteResp { term, vote_granted } = request_vote_resp;
        let term_matches = context.raft_state.current_term == term;

        if term_matches && vote_granted {
            //% Compliance:
            //% wins election
            //%	- receives majority of votes in cluster (ensures a single winner)
            //%	- a server can only vote once for a given term (first-come basis)
            //%	- a candidate becomes `leader` if it wins the election
            //%	- sends a heartbeat to establish itself as a leader and prevent a new election
            let granted_vote = self.on_vote_received(peer_id, context);
            if matches!(granted_vote, ElectionResult::Elected) {
                //% Compliance:
                //% If votes received from majority of servers: become leader
                return ModeTransition::ToLeader;
            }
        }

        ModeTransition::None
    }

    fn start_election<E: ServerEgress>(&mut self, context: &mut Context<E>) -> ModeTransition {
        //% Compliance:
        //% Increment currentTerm
        context.raft_state.current_term.increment();

        //% Compliance:
        //% Vote for self
        if matches!(
            self.on_vote_received(context.server_id, context),
            ElectionResult::Elected
        ) {
            //% Compliance:
            //% If votes received from majority of servers: become leader
            return ModeTransition::ToLeader;
        }

        //% Compliance:
        //% Reset election timer
        context.raft_state.election_timer.reset();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        let current_term = context.raft_state.current_term;
        //% Compliance:
        //% Send RequestVote RPCs to all other servers
        for (_id, peer) in context.peer_map.iter_mut() {
            let prev_log_term_idx = context.raft_state.peers_prev_term_idx(peer);
            Rpc::new_request_vote(current_term, context.server_id, prev_log_term_idx)
                .encode_mut(&mut buf);
            peer.send(buf.as_mut_slice().to_vec());
        }

        ModeTransition::None
    }

    fn on_vote_received<E: ServerEgress>(
        &mut self,
        voter_id: ServerId,
        context: &Context<E>,
    ) -> ElectionResult {
        debug_assert!(
            context.peer_map.contains_key(&voter_id) || voter_id == context.server_id,
            "voter id not a raft peer"
        );
        self.votes_received.insert(voter_id);

        if self.votes_received.len() >= Mode::quorum(context) {
            ElectionResult::Elected
        } else {
            ElectionResult::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        log::{Term, TermIdx},
        mode::cast_unsafe,
        peer::Peer,
        raft_state::RaftState,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::DecoderBuffer;

    #[tokio::test]
    async fn test_start_election() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer_fill = 11;
        let server_id = ServerId::new([peer_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer_fill, 12]);
        let mut state = RaftState::new(timeout, &peer_map);
        assert!(state.current_term.is_initial());

        let mut context = Context {
            server_id,
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut candidate = Candidate::default();

        // Trigger election
        let transition = candidate.start_election(&mut context);

        // Expect no transitions since quorum is >1
        assert!(matches!(transition, ModeTransition::None));
        // Expect current_term to be incremented
        assert_eq!(state.current_term, Term::from(1));

        // Expect RequestVote RPC sent to all peers
        let expected_rpc = Rpc::new_request_vote(state.current_term, server_id, TermIdx::initial());
        for (_id, peer) in peer_map.iter_mut() {
            let Peer { id: _, io_egress } = peer;
            let rpc_bytes = io_egress.send_queue.pop().unwrap();
            let buffer = DecoderBuffer::new(&rpc_bytes);
            let (sent_request_vote, _) = buffer.decode::<Rpc>().unwrap();
            assert_eq!(expected_rpc, sent_request_vote);
        }
    }

    #[tokio::test]
    async fn test_start_election_with_no_peers() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([6; 16]);
        let mut peer_map = Peer::mock_as_map(&[]);
        let mut state = RaftState::new(timeout, &peer_map);
        let mut context = Context {
            server_id,
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut candidate = Candidate::default();
        assert_eq!(Mode::quorum(&context), 1);

        // Elect self
        let transition = candidate.start_election(&mut context);
        assert!(matches!(transition, ModeTransition::ToLeader));

        // No RPC sent. Unable to inspect E since there are no peers
        assert!(peer_map.first_entry().is_none());
    }

    #[tokio::test]
    async fn test_vote_received() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let self_id = ServerId::new([1; 16]);
        let peer2_fill = 2;
        let peer2_id = ServerId::new([peer2_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer2_fill, 3]);
        let mut state = RaftState::new(timeout, &peer_map);

        let context = Context {
            server_id: self_id,
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut candidate = Candidate::default();
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

    #[tokio::test]
    async fn test_recv_request_vote_resp_term() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer_id2_fill = 2;
        let peer_id2 = ServerId::new([peer_id2_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer_id2_fill]);
        let mut state = RaftState::new(timeout, &peer_map);

        let term_current = Term::from(2);
        let term_election = term_current + 1;
        state.current_term = term_current;

        let mut context = Context {
            server_id,
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        assert_eq!(Mode::quorum(&context), 2);

        // Initialize Candidate (votes for self)
        let mut candidate = Candidate::default();
        assert_eq!(candidate.votes_received.len(), 0);
        let transition = candidate.start_election(&mut context);
        assert!(matches!(transition, ModeTransition::None));
        assert_eq!(candidate.votes_received.len(), 1);

        // grant and no_grant vote RPC

        // peer 2 DOES grant vote but has older Term
        let prev_term_rpc = cast_unsafe!(
            Rpc::new_request_vote_resp(term_current, true),
            Rpc::RequestVoteResp
        );
        let transition = candidate.on_recv_request_vote_resp(peer_id2, prev_term_rpc, &mut context);
        assert!(matches!(transition, ModeTransition::None));
        assert_eq!(candidate.votes_received.len(), 1);

        // peer 2 DOES grant vote but has election Term
        let election_term_rpc = cast_unsafe!(
            Rpc::new_request_vote_resp(term_election, true),
            Rpc::RequestVoteResp
        );
        let transition =
            candidate.on_recv_request_vote_resp(peer_id2, election_term_rpc, &mut context);
        assert!(matches!(transition, ModeTransition::ToLeader));
        assert_eq!(candidate.votes_received.len(), 2);
    }

    #[tokio::test]
    async fn test_recv_request_vote_resp_grant_vote() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer_id2_fill = 2;
        let peer_id2 = ServerId::new([peer_id2_fill; 16]);
        let peer_id3_fill = 3;
        let peer_id3 = ServerId::new([peer_id3_fill; 16]);
        let peer_id4_fill = 4;
        let mut peer_map = Peer::mock_as_map(&[peer_id2_fill, peer_id3_fill, peer_id4_fill]);
        let mut state = RaftState::new(timeout, &peer_map);

        let term_current = Term::from(2);
        let term_election = term_current + 1;
        state.current_term = term_current;

        let mut context = Context {
            server_id,
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        assert_eq!(Mode::quorum(&context), 3);

        // Initialize Candidate (votes for self)
        let mut candidate = Candidate::default();
        assert_eq!(candidate.votes_received.len(), 0);
        let transition = candidate.start_election(&mut context);
        assert!(matches!(transition, ModeTransition::None));
        assert_eq!(candidate.votes_received.len(), 1);

        // grant and no_grant vote RPC
        let grant_vote_rpc = cast_unsafe!(
            Rpc::new_request_vote_resp(term_election, true),
            Rpc::RequestVoteResp
        );
        let no_grant_vote_rpc = cast_unsafe!(
            Rpc::new_request_vote_resp(term_election, false),
            Rpc::RequestVoteResp
        );

        // peer 2 DOES grant vote
        let transition =
            candidate.on_recv_request_vote_resp(peer_id2, grant_vote_rpc.clone(), &mut context);
        assert!(matches!(transition, ModeTransition::None));
        assert_eq!(candidate.votes_received.len(), 2);

        // peer 3 does NOT grant vote
        let transition =
            candidate.on_recv_request_vote_resp(peer_id3, no_grant_vote_rpc, &mut context);
        assert!(matches!(transition, ModeTransition::None));
        assert_eq!(candidate.votes_received.len(), 2);

        // peer 3 DOES grant vote
        let transition =
            candidate.on_recv_request_vote_resp(peer_id3, grant_vote_rpc, &mut context);
        assert!(matches!(transition, ModeTransition::ToLeader));
        assert_eq!(candidate.votes_received.len(), 3);
    }
}
