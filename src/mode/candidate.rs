use crate::{
    io::ServerEgress,
    mode::{cast_unsafe, ElectionResult, Mode, ModeTransition},
    raft_state::RaftState,
    rpc::{AppendEntries, RequestVoteResp, Rpc},
    server::{Id, PeerId, ServerId},
};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct Candidate {
    votes_received: HashSet<Id>,
}

impl Candidate {
    pub fn on_candidate<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) -> ModeTransition {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(server_id, peer_list, raft_state, io_egress)
    }

    pub fn on_timeout<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) -> ModeTransition {
        //% Compliance:
        //% A timeout occurs and there is no winner (can happen if too many servers become
        //% candidates at the same time)
        //% - increment its term
        //% - start a new election by initiating another round of RequestVote
        //
        //% Compliance:
        //% If election timeout elapses: start new election
        self.start_election(server_id, peer_list, raft_state, io_egress)
    }

    pub fn on_recv<'a, E: ServerEgress>(
        &mut self,
        peer_id: PeerId,
        rpc: &'a Rpc,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) -> (ModeTransition, Option<&'a Rpc>) {
        match rpc {
            Rpc::RequestVote(request_vote) => {
                request_vote.on_recv(peer_id, raft_state, io_egress);
                (ModeTransition::Noop, None)
            }
            Rpc::RequestVoteResp(request_vote_resp) => {
                let transition = self.on_recv_request_vote_resp(
                    peer_id,
                    request_vote_resp,
                    peer_list,
                    raft_state,
                );
                (transition, None)
            }
            rpc @ Rpc::AppendEntry(_append_entries) => {
                self.on_recv_append_entries(peer_id, rpc, raft_state, io_egress)
            }
            Rpc::AppendEntryResp(_) => {
                todo!("it might be possible to get a response from a previous term")
            }
        }
    }

    fn on_recv_append_entries<'a, E: ServerEgress>(
        &mut self,
        peer_id: PeerId,
        rpc: &'a Rpc,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) -> (ModeTransition, Option<&'a Rpc>) {
        let append_entries = cast_unsafe!(rpc, Rpc::AppendEntry);
        let AppendEntries {
            term,
            leader_id: _,
            prev_log_term_idx: _,
            leader_commit_idx: _,
            entries: _,
        } = append_entries;
        //% Compliance:
        //% another server establishes itself as a leader
        //% - a candidate receives AppendEntries from another server claiming to be a leader
        if term >= &raft_state.current_term {
            //% Compliance:
            //% if that leader's current term is >= the candidate's
            //% - recognize the server as the new leader
            //% - then the candidate reverts to a follower
            //
            //% Compliance:
            //% If AppendEntries RPC received from new leader: convert to follower

            // Convert to Follower and process/respond to the RPC
            (ModeTransition::ToFollower, Some(rpc))
        } else {
            //% Compliance:
            //% if the leader's current term is < the candidate's
            //% - reject the RPC and continue in the candidate state
            let term = raft_state.current_term;
            let rpc = Rpc::new_append_entry_resp(term, false);
            let leader_io = io_egress;
            leader_io.send_packet(peer_id, rpc);
            (ModeTransition::Noop, None)
        }
    }

    fn on_recv_request_vote_resp(
        &mut self,
        peer_id: PeerId,
        request_vote_resp: &RequestVoteResp,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
    ) -> ModeTransition {
        let RequestVoteResp { term, vote_granted } = request_vote_resp;
        let term_matches = raft_state.current_term.eq(term);

        if term_matches && *vote_granted {
            //% Compliance:
            //% wins election
            //%	- receives majority of votes in cluster (ensures a single winner)
            //%	- a server can only vote once for a given term (first-come basis)
            //%	- a candidate becomes `leader` if it wins the election
            //%	- sends a heartbeat to establish itself as a leader and prevent a new election
            let granted_vote = self.on_vote_received(&peer_id, peer_list);
            if matches!(granted_vote, ElectionResult::Elected) {
                //% Compliance:
                //% If votes received from majority of servers: become leader
                return ModeTransition::ToLeader;
            }
        }

        ModeTransition::Noop
    }

    fn start_election<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) -> ModeTransition {
        // Retrieve the last log term index before incrementing the currentTerm
        let last_log_term_idx = raft_state.on_start_election();

        //% Compliance:
        //% Reset election timer
        raft_state.election_timer.reset();

        //% Compliance:
        //% Vote for self
        if matches!(
            self.on_vote_for_self(server_id, peer_list),
            ElectionResult::Elected
        ) {
            //% Compliance:
            //% If votes received from majority of servers: become leader
            return ModeTransition::ToLeader;
        }

        //% Compliance:
        //% Send RequestVote RPCs to all other servers
        for peer in peer_list.iter() {
            let rpc = Rpc::new_request_vote(raft_state.current_term, *server_id, last_log_term_idx);
            peer.send_rpc(rpc, io_egress);
        }

        ModeTransition::Noop
    }

    fn on_vote_for_self(&mut self, server_id: &ServerId, peer_list: &[PeerId]) -> ElectionResult {
        debug_assert!(
            !peer_list
                .iter()
                .any(|&x| x.as_bytes().eq(server_id.as_bytes())),
            "vote_for_self should not be called with a peer_id"
        );
        self.votes_received.insert(server_id.into_id());
        self.check_election_result(peer_list)
    }

    fn on_vote_received(&mut self, peer_id: &PeerId, peer_list: &[PeerId]) -> ElectionResult {
        debug_assert!(
            peer_list
                .iter()
                .any(|x| x.as_bytes().eq(peer_id.as_bytes())),
            "voter id should be a peer"
        );
        self.votes_received.insert(peer_id.into_id());
        self.check_election_result(peer_list)
    }

    fn check_election_result(&mut self, peer_list: &[PeerId]) -> ElectionResult {
        if self.votes_received.len() >= Mode::quorum(peer_list) {
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
        io::testing::{helper_inspect_next_sent_packet, MockIo},
        log::{Term, TermIdx},
        raft_state::RaftState,
        server::PeerId,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn test_start_election() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer_id = PeerId::new([11; 16]);
        let peer_id_2 = PeerId::new([12; 16]);
        let mut peer_list = vec![peer_id, peer_id_2];
        let mut state = RaftState::new(timeout);
        assert!(state.current_term.is_initial());

        let mut io = MockIo::new(server_id);
        let mut candidate = Candidate::default();

        // Trigger election
        let transition = candidate.start_election(&server_id, &peer_list, &mut state, &mut io);

        // Expect no transitions since quorum is >1
        assert!(matches!(transition, ModeTransition::Noop));
        // Expect current_term to be incremented
        assert_eq!(state.current_term, Term::from(1));

        // Expect RequestVote RPC sent to all peers
        let expected_rpc = Rpc::new_request_vote(state.current_term, server_id, TermIdx::initial());
        // TODO assert which peer we are sending to
        for _peer in peer_list.iter_mut() {
            let packet = helper_inspect_next_sent_packet(&mut io);
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    #[tokio::test]
    async fn test_start_election_with_no_peers() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([6; 16]);
        let peer_list = vec![];
        let mut state = RaftState::new(timeout);

        let mut io = MockIo::new(server_id);
        let mut candidate = Candidate::default();
        assert_eq!(Mode::quorum(&peer_list), 1);

        // Elect self
        let transition = candidate.start_election(&server_id, &peer_list, &mut state, &mut io);
        assert!(matches!(transition, ModeTransition::ToLeader));

        // No RPC sent. Unable to inspect E since there are no peers
        assert!(peer_list.is_empty());
    }

    #[tokio::test]
    async fn test_vote_received() {
        let self_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];

        let mut candidate = Candidate::default();
        assert_eq!(Mode::quorum(&peer_list), 2);
        assert!(Mode::quorum(&peer_list) > candidate.votes_received.len());

        // Receive peer's vote
        assert!(matches!(
            candidate.on_vote_received(&peer2_id, &peer_list),
            ElectionResult::Pending
        ));
        assert!(Mode::quorum(&peer_list) > candidate.votes_received.len());

        // Don't count same vote
        assert!(matches!(
            candidate.on_vote_received(&peer2_id, &peer_list),
            ElectionResult::Pending
        ));
        assert!(Mode::quorum(&peer_list) > candidate.votes_received.len());
        //
        // Vote for self and reach quorum
        assert!(matches!(
            candidate.on_vote_for_self(&self_id, &peer_list),
            ElectionResult::Elected
        ));
        assert_eq!(Mode::quorum(&peer_list), 2);
    }

    #[tokio::test]
    async fn test_recv_request_vote_resp_term() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer_list = vec![peer2_id];
        let mut state = RaftState::new(timeout);

        let term_current = Term::from(2);
        let term_election = term_current + 1;
        state.current_term = term_current;

        let mut io = MockIo::new(server_id);
        assert_eq!(Mode::quorum(&peer_list), 2);

        // Initialize Candidate (votes for self)
        let mut candidate = Candidate::default();
        assert_eq!(candidate.votes_received.len(), 0);
        let transition = candidate.start_election(&server_id, &peer_list, &mut state, &mut io);
        assert!(matches!(transition, ModeTransition::Noop));
        assert_eq!(candidate.votes_received.len(), 1);

        // grant and no_grant vote RPC

        // peer 2 DOES grant vote but has older Term
        let prev_term_rpc = cast_unsafe!(
            Rpc::new_request_vote_resp(term_current, true),
            Rpc::RequestVoteResp
        );
        let transition =
            candidate.on_recv_request_vote_resp(peer2_id, &prev_term_rpc, &peer_list, &mut state);
        assert!(matches!(transition, ModeTransition::Noop));
        assert_eq!(candidate.votes_received.len(), 1);

        // peer 2 DOES grant vote but has election Term
        let election_term_rpc = cast_unsafe!(
            Rpc::new_request_vote_resp(term_election, true),
            Rpc::RequestVoteResp
        );
        let transition = candidate.on_recv_request_vote_resp(
            peer2_id,
            &election_term_rpc,
            &peer_list,
            &mut state,
        );
        assert!(matches!(transition, ModeTransition::ToLeader));
        assert_eq!(candidate.votes_received.len(), 2);
    }

    #[tokio::test]
    async fn test_recv_request_vote_resp_grant_vote() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer4_id = PeerId::new([13; 16]);
        let peer_list = vec![peer2_id, peer3_id, peer4_id];
        let mut state = RaftState::new(timeout);

        let term_current = Term::from(2);
        let term_election = term_current + 1;
        state.current_term = term_current;

        let mut io = MockIo::new(server_id);
        assert_eq!(Mode::quorum(&peer_list), 3);

        // Initialize Candidate (votes for self)
        let mut candidate = Candidate::default();
        assert_eq!(candidate.votes_received.len(), 0);
        let transition = candidate.start_election(&server_id, &peer_list, &mut state, &mut io);
        assert!(matches!(transition, ModeTransition::Noop));
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
            candidate.on_recv_request_vote_resp(peer2_id, &grant_vote_rpc, &peer_list, &mut state);
        assert!(matches!(transition, ModeTransition::Noop));
        assert_eq!(candidate.votes_received.len(), 2);

        // peer 3 does NOT grant vote
        let transition = candidate.on_recv_request_vote_resp(
            peer3_id,
            &no_grant_vote_rpc,
            &peer_list,
            &mut state,
        );
        assert!(matches!(transition, ModeTransition::Noop));
        assert_eq!(candidate.votes_received.len(), 2);

        // peer 3 DOES grant vote
        let transition =
            candidate.on_recv_request_vote_resp(peer3_id, &grant_vote_rpc, &peer_list, &mut state);
        assert!(matches!(transition, ModeTransition::ToLeader));
        assert_eq!(candidate.votes_received.len(), 3);
    }
}
