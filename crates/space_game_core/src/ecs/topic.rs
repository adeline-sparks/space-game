use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

pub trait Topic: 'static {}

pub struct AnyTopic(Box<dyn Any>);

impl AnyTopic {
    pub fn new<T: Topic>(t: T) -> Self {
        Self(Box::new(t))
    }

    pub fn type_id(&self) -> TypeId {
        self.0.type_id()
    }

    pub fn downcast<'a, T: Topic>(&'a self) -> &'a T {
        self.0.downcast_ref().unwrap() // TODO
    }
}

#[derive(Default)]
pub struct TopicContainer(RefCell<HashMap<TypeId, Vec<AnyTopic>>>);

impl TopicContainer {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn publish<T: Topic>(&self, t: T) {
        self.0
            .borrow_mut()
            .entry(TypeId::of::<T>())
            .or_default()
            .push(AnyTopic::new(t));
    }

    pub fn get<T: Topic>(&self, idx: usize) -> Option<Ref<'_, T>> {
        let tid = TypeId::of::<T>();
        if self.0.borrow().get(&tid).map(|v| idx < v.len()) != Some(true) {
            return None;
        }

        Some(Ref::map(self.0.borrow(), |m| m[&tid][idx].downcast::<T>()))
    }

    pub fn clear(&self) {
        for v in self.0.borrow_mut().values_mut() {
            v.clear();
        }
    }
}

pub struct Publisher<'t, T: Topic>(&'t TopicContainer, PhantomData<&'t T>);

impl<'t, T: Topic> Publisher<'t, T> {
    pub fn publish(&self, t: T) {
        self.0.publish(t);
    }
}

impl<'t, T: Topic> HandlerFnArg for Publisher<'t, T> {
    type Builder = PublisherBuilder<T>;

    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::PublishTopic(TypeId::of::<T>())]
    }
}

pub struct PublisherBuilder<T>(PhantomData<T>);

impl<'c, T: Topic> HandlerFnArgBuilder<'c> for PublisherBuilder<T> {
    type Arg = Publisher<'c, T>;

    fn build(context: &'c Context) -> Self::Arg {
        Publisher(&context.topics, PhantomData)
    }
}

pub struct Subscriber<'t, T: Topic>(&'t TopicContainer, PhantomData<&'t T>);

impl<'t, T: Topic> Subscriber<'t, T> {
    pub fn iter<'a>(&'a self) -> impl Iterator + 'a {
        (0..).into_iter().map_while(move |idx| self.0.get::<T>(idx))
    }
}

impl<'t, T: Topic> HandlerFnArg for Subscriber<'t, T> {
    type Builder = SubscriberBuilder<T>;

    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::SubscribeTopic(TypeId::of::<T>())]
    }
}

pub struct SubscriberBuilder<T>(PhantomData<T>);

impl<'c, T: Topic> HandlerFnArgBuilder<'c> for SubscriberBuilder<T> {
    type Arg = Subscriber<'c, T>;

    fn build(context: &'c Context) -> Subscriber<'c, T> {
        Subscriber(&context.topics, PhantomData)
    }
}
