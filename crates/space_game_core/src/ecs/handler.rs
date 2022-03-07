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
    fn into_handler(self) -> Handler;
}

pub trait HandlerFnArg {
    type Builder: for<'c> HandlerFnArgBuilder<'c>;

    fn dependencies(out: &mut Vec<Dependency>);
}

pub trait HandlerFnArgBuilder<'c> {
    type Arg: HandlerFnArg;

    fn build(context: &'c Context) -> Self::Arg;
}

macro_rules! impl_handler_fn {
    ($($Args:ident),*) => {
        impl<E, $($Args,)* F> HandlerFn<E, ($($Args,)*)> for F where
            E: Event,
            $($Args: HandlerFnArg,)*
            F: 'static,
            for<'f> &'f F: Fn(&E, $($Args,)*) -> anyhow::Result<()>,
            for<'f> &'f F: Fn(&E, $(<$Args::Builder as HandlerFnArgBuilder>::Arg,)*) -> anyhow::Result<()>,
        {
            fn into_handler(self) -> Handler {
                fn make_fn<E, $($Args,)*>(
                    f: impl Fn(&E, $($Args,)*) -> anyhow::Result<()>
                ) -> impl Fn(&E, $($Args,)*) -> anyhow::Result<()> {
                    f
                }

                Handler {
                    event_id: E::id(),
                    dependencies: {
                        #[allow(unused_mut)]
                        let mut result = Vec::new();
                        $($Args::dependencies(&mut result);)*
                        result
                    },
                    fn_box: Box::new(move |context| {
                        make_fn(&self)(context.event.downcast(), $($Args::Builder::build(context),)*)
                    }),
                }
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
