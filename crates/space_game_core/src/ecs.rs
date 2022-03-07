mod event;
mod handler;
mod reactor;
mod state;
mod topic;

pub use event::{AnyEvent, Event, EventWriter};
pub use handler::{Handler, HandlerFn};
pub use reactor::{InitState, Reactor};
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

        fn handler1(ev: &MyEvent, mut state: Writer<'_, MyState>) -> anyhow::Result<()> {
            state.sum += ev.counter;
            Ok(())
        }

        fn handler2(ev: &MyEvent, ev_write: EventWriter<'_>) -> anyhow::Result<()> {
            if ev.counter > 0 {
                ev_write.write(MyEvent {
                    counter: ev.counter - 1,
                });
                ev_write.write(MyEvent {
                    counter: ev.counter - 1,
                });
            }

            Ok(())
        }

        let reactor = Reactor::builder().add(handler1).add(handler2).build();

        let states = reactor.new_state().unwrap();
        assert!(reactor.dispatch(&states, MyEvent { counter: 5 }).is_ok());
        assert_eq!(
            states.get::<MyState>().sum,
            1 * 5 + 2 * 4 + 4 * 3 + 8 * 2 + 16 * 1
        );
    }
}
