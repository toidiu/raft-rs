# raft-rs

A toy implementation to better understand the raft consensus protocol

## High-level
- replicated state machine
- durable write ahead log

- role: leader vs follower
- struct: state_machine { role,  }
- enum: operations

## Based on TIKV impl: https://github.com/tikv/raft-rs

> A complete Raft model contains 4 essential parts:
> - Consensus Module, the core consensus algorithm module;
> - Log, the place to keep the Raft logs;
> - State Machine, the place to save the user data;
> - Transport, the network layer for communication.

