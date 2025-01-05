use crate::state::{Action, ServerTx};

#[derive(Default)]
pub struct FollowerState;

impl Action for FollowerState {
    fn on_convert<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    fn on_timeout<T: ServerTx>(&mut self, _io: &mut T) {
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        io::testing::MockTx,
        state::{Mode, State},
    };

    #[tokio::test]
    async fn follower_timout() {
        let mut s = State::new();
        assert!(matches!(s.mode, Mode::Follower(_)));
        s.on_timeout(&mut MockTx);
    }
}
