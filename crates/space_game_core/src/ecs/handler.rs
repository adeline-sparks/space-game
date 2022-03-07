use impl_trait_for_tuples::impl_for_tuples;

use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::state::{StateContainer, StateId};
use super::topic::{TopicContainer, TopicId};

pub struct Handler {
    event_id: EventId,
    dependencies: Vec<Dependency>,
    fn_box: Box<dyn Fn(&Context) -> anyhow::Result<()>>,
}

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum Dependency {
    ReadState(StateId),
    ReadStateDelayed(StateId),
    WriteState(StateId),
    SubscribeTopic(TopicId),
    PublishTopic(TopicId),
}

pub struct Context<'a> {
    pub states: &'a StateContainer,
    pub queue: &'a EventQueue,
    pub topics: &'a TopicContainer,
    pub event: &'a AnyEvent,
}

impl Handler {
    pub fn new<E: Event, Args, F: HandlerFn<E, Args> + 'static>(f: F) -> Self {
        let mut dependencies = Vec::new();
        F::dependencies(&mut dependencies);
        Handler {
            event_id: F::event_id(),
            dependencies: {
                let mut result = Vec::new();
                F::dependencies(&mut result);
                result
            },
            fn_box: Box::new(move |context| f.call(context)),
        }
    }

    pub fn event_id(&self) -> EventId {
        self.event_id
    }

    pub fn dependencies(&self) -> &[Dependency] {
        &*self.dependencies
    }

    pub fn call(&self, context: &Context) -> anyhow::Result<()> {
        (self.fn_box)(context)
    }
}

pub trait HandlerFn<E, Args> {
    fn event_id() -> EventId;
    fn dependencies(out: &mut Vec<Dependency>);

    fn call(&self, context: &Context) -> anyhow::Result<()>;
}

pub trait HandlerFnArg {
    type Builder: for<'c> HandlerFnArgBuilder<'c>;
    fn dependencies(out: &mut Vec<Dependency>);
}

pub trait HandlerFnArgBuilder<'c> {
    type Arg;

    fn build(context: &'c Context) -> Self::Arg;
}

macro_rules! impl_handler_fn {
    ($($Args:ident),*) => {
        impl<E, $($Args,)* F> HandlerFn<E, ($($Args,)*)> for F where
            E: Event,
            $($Args: HandlerFnArg,)*
            F: Fn(&E, $($Args,)*) -> anyhow::Result<()>,
            F: Fn(&E, $(<$Args::Builder as HandlerFnArgBuilder>::Arg,)*) -> anyhow::Result<()>,
        {
            fn event_id() -> EventId {
                E::id()
            }

            fn dependencies(out: &mut Vec<Dependency>) {
                let _ = out;
                $($Args::dependencies(out));*
            }

            fn call(&self, context: &Context) -> anyhow::Result<()> {
                (self)(context.event.downcast(), $($Args::Builder::build(context),)*)
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

#[impl_for_tuples(5)]
impl HandlerFnArg for Tuple {
    for_tuples!(type Builder = ( #(Tuple::Builder),* ); );

    fn dependencies(out: &mut Vec<Dependency>) {
        for_tuples!(#(Tuple::dependencies(out);)*);
    }
}

#[impl_for_tuples(5)]
impl<'c> HandlerFnArgBuilder<'c> for Tuple {
    for_tuples!(where #(Tuple: HandlerFnArgBuilder<'c>)* );

    for_tuples!(type Arg = (#(Tuple::Arg),*); );

    fn build(context: &'c Context) -> Self::Arg {
        for_tuples!((#(Tuple::build(context)),*))
    }
}
