# raft-rs

A toy implementation to better understand the raft consensus protocol

## High-level

Based on TIKV impl: https://github.com/tikv/raft-rs

> A complete Raft model contains 4 essential parts:
>
> - Consensus Module, the core consensus algorithm module;
> - Log, the place to keep the Raft logs;
> - State Machine, the place to save the user data;
> - Transport, the network layer for communication.

`struct StateMachine`

"Replicated state machine". The concept that the same data is spread over many
machines so the failure of minority of machines doesnt impact liveliness.

`struct ReplicatedLog`

Replicated state machines are implemented via a "replicated log", which are a serive of
commands to be executed in-order.

`struct ConsensusAlgo`

The job of the "consensus algorithm" is keeping the replicated log consistent.

The consensus algo on a server receives commands from a client and adds them it its
log. It then commicumates with other consensus algo on other servers to ensure that
every log contains the same commands in the same order.

3 properties
- Leader Election
- Log Repliation: leader manages replicated log
- Safety: entries added to the state machine are absolute and correct
