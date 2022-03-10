use std::any::type_name;
use std::fmt::{Debug, Display};
use std::panic::Location;

use anyhow::bail;
use impl_trait_for_tuples::impl_for_tuples;

use super::dependency::Dependency;
use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::state::StateContainer;
use super::topic::TopicContainer;

pub struct Handler {
    event_id: EventId,
    dependencies: Vec<Dependency>,
    fn_box: Box<dyn Fn(&Context) -> anyhow::Result<()>>,
    name: Option<String>,
    location: Location<'static>,
}

impl Debug for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handler")
            .field("event_id", &self.event_id)
            .field("dependencies", &self.dependencies)
            .field("fn_box", &())
            .field("name", &self.name)
            .field("location", &self.location)
            .finish()
    }
}

impl Display for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({})",
            self.name.as_deref().unwrap_or("Unnamed handler"),
            self.location,
        )
    }
}

pub struct Context<'a> {
    pub states: &'a StateContainer,
    pub queue: &'a EventQueue,
    pub topics: &'a TopicContainer,
    pub event: &'a AnyEvent,
}

impl Handler {
    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn dependencies(&self) -> &[Dependency] {
        &*self.dependencies
    }

    pub fn call(&self, context: &Context) -> anyhow::Result<()> {
        (self.fn_box)(context)
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn location(&self) -> &Location<'static> {
        &self.location
    }
}

pub trait HandlerFn<E, Args> {
    #[track_caller]
    fn into_handler(self) -> Handler;
}

pub trait HandlerFnArg {
    type Builder: for<'c> HandlerFnArgBuilder<'c>;

    fn dependencies(out: &mut Vec<Dependency>);
}

pub trait HandlerFnArgBuilder<'c> {
    type Arg: HandlerFnArg;

    fn build(context: &'c Context) -> anyhow::Result<Self::Arg>;
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
            #[track_caller]
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
                        if let Some(event) = context.event.downcast() {
                            make_fn(&self)(event, $($Args::Builder::build(context)?,)*)
                        } else {
                            let expected = type_name::<E>();
                            let actual = context.event.type_name();
                            bail!("Handler called with invalid event: expected `{expected}` but given `{actual}`")
                        }
                    }),
                    name: None,
                    location: Location::caller().clone(),
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

    fn build(context: &'c Context) -> anyhow::Result<Self::Arg> {
        Ok(for_tuples!((#(Tuple::build(context)?),*)))
    }
}
