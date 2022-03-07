mod event;
mod handler;
mod reactor;
mod state;
mod topic;

pub use event::{AnyEvent, Event, EventWriter};
pub use handler::{Handler, HandlerFn};
pub use reactor::Reactor;
pub use state::{DelayedReader, Reader, State, StateContainer, Writer};
pub use topic::{Publisher, Subscriber, Topic};

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simple() {
        #[derive(Clone, Default)]
        struct MyState {
            sum: usize,
        }
        impl State for MyState {}

        #[derive(Clone, Default)]
        struct MyStateCopy(MyState);
        impl State for MyStateCopy {}

        struct MyEvent {
            counter: usize,
        }
        impl Event for MyEvent {}

        let mut states = StateContainer::new();
        states.insert_default::<MyState>();
        states.insert_default::<MyStateCopy>();

        let handler1 = Handler::new(
            |ev: &MyEvent, mut state: Writer<'_, MyState>| -> anyhow::Result<()> {
                state.sum += ev.counter;
                Ok(())
            },
        );

        let handler2 = Handler::new(
            |ev: &MyEvent, ev_write: EventWriter<'_>| -> anyhow::Result<()> {
                if ev.counter > 0 {
                    ev_write.write(MyEvent {
                        counter: ev.counter - 1,
                    });
                    ev_write.write(MyEvent {
                        counter: ev.counter - 1,
                    });
                }
                Ok(())
            },
        );

        let reactor = Reactor::new(states, vec![handler1, handler2]);
        assert!(reactor.dispatch(MyEvent { counter: 5 }).is_ok());
        assert_eq!(
            reactor.states().get::<MyState>().unwrap().sum,
            1 * 5 + 2 * 4 + 4 * 3 + 8 * 2 + 16 * 1
        );
    }
}
