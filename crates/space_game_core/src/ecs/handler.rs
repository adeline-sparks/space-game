use once_cell::sync::{OnceCell};

use crate::{ecs::event::{AnyEvent, Event}, ecs::state::StateContainer};

use super::{reactor::Dependency, event::EventQueue, topic::TopicContainer};

pub struct Handler {
    pub dependencies: &'static [Dependency],
    pub fn_box: Box<dyn Fn(&AnyEvent, &StateContainer, &EventQueue, &TopicContainer) -> anyhow::Result<()>>,
}

impl Handler {
    pub fn new<E, Args, F: HandlerFn<E, Args> + 'static>(f: F) -> Self {
        Handler {
            dependencies: F::dependencies(),
            fn_box: Box::new(move |ev, world, queue, topics| f.call(ev, world, queue, topics)),
        }
    }
}

pub trait HandlerFn<E, Args> {
    fn dependencies() -> &'static [Dependency];

    fn call(&self, ev: &AnyEvent, world: &StateContainer, queue: &EventQueue, topics: &TopicContainer) -> anyhow::Result<()>;
}

pub trait HandlerFnArg<'a> {
    fn dependency() -> Option<Dependency>;

    fn build(world: &'a StateContainer, queue: &'a EventQueue, topics: &'a TopicContainer) -> Self;
}

macro_rules! impl_handler_fn {
    ($($Args:ident),*) => {
        impl<E, $($Args,)* F> HandlerFn<E, ($($Args,)*)> for F where
            E: Event,
            $($Args: for <'a> HandlerFnArg<'a>,)*
            F: Fn(&E, $($Args,)*) -> anyhow::Result<()>,
        {
            fn dependencies() -> &'static [Dependency] { 
                static DEPS: OnceCell<Vec<Dependency>> = OnceCell::new();
                if let Some(vec) = DEPS.get() { 
                    return vec.as_slice();
                }

                let deps: &[Option<Dependency>] = &[$($Args::dependency(),)*];
        
                let _ = DEPS.set(deps.into_iter().flatten().cloned().collect());
                DEPS.get().unwrap()
            }
        
            fn call(&self, ev: &AnyEvent, states: &StateContainer, events: &EventQueue, topics: &TopicContainer) -> anyhow::Result<()> {
                (self)(ev.downcast(), $($Args::build(states, events, topics),)*)
            }
        }
    }
}

impl_handler_fn!();
impl_handler_fn!(A1);
impl_handler_fn!(A1, A2);
impl_handler_fn!(A1, A2, A3);
impl_handler_fn!(A1, A2, A3, A4);
impl_handler_fn!(A1, A2, A3, A4, A5);
