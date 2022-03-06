use std::{any::{Any, TypeId}, cell::{RefCell, Ref, RefMut}, collections::HashMap, marker::PhantomData};

use super::{handler::HandlerFnArg, reactor::Dependency, state::StateContainer};

pub trait Topic: 'static { }

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
        self.0.borrow_mut().entry(TypeId::of::<T>()).or_default().push(AnyTopic::new(t));
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

impl<'t, T: Topic> HandlerFnArg<'t> for Publisher<'t, T> {
    fn dependency() -> Option<super::reactor::Dependency> {
        Some(Dependency::PublishTopic(TypeId::of::<T>()))
    }

    fn build(_world: &'t StateContainer, _events: &'t super::event::EventQueue, topics: &'t TopicContainer) -> Self {
        Publisher(topics, PhantomData)
    }
}

pub struct Subscriber<'t, T: Topic>(&'t TopicContainer, PhantomData<&'t T>);

impl<'t, T: Topic> Subscriber<'t, T> {
    pub fn iter<'a>(&'a self) -> impl Iterator + 'a {
        (0..).into_iter().map_while(move |idx| self.0.get::<T>(idx))
    }
}

impl<'t, T: Topic> HandlerFnArg<'t> for Subscriber<'t, T> {
    fn dependency() -> Option<super::reactor::Dependency> {
        Some(Dependency::SubscribeTopic(TypeId::of::<T>()))
    }

    fn build(world: &'t super::state::StateContainer, events: &'t super::event::EventQueue, topics: &'t TopicContainer) -> Self {
        Subscriber(topics, PhantomData)
    }
}

