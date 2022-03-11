use std::any::{type_name, Any, TypeId};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt::{self, Debug, Display};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

pub trait Topic: Debug + 'static {
    fn id() -> TopicId {
        TopicId {
            id: TypeId::of::<Self>(),
            name: type_name::<Self>(),
        }
    }
}

#[derive(Eq, Clone, Debug)]
pub struct TopicId {
    id: TypeId,
    name: &'static str,
}

impl PartialEq for TopicId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for TopicId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// Dynamically-typed container for types that implement [`Event`].
pub struct AnyTopic(Box<dyn AnyTopicInner>);

/// Object-safe trait used inside [`AnyTopic`].
trait AnyTopicInner {
    fn as_any(&self) -> &dyn Any;
    fn id(&self) -> TopicId;
    fn debug_fmt(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

impl<T: Topic + Sized> AnyTopicInner for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn id(&self) -> TopicId {
        T::id()
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl AnyTopic {
    pub fn new<T: Topic>(t: T) -> Self {
        Self(Box::new(t))
    }

    pub fn id(&self) -> TopicId {
        self.0.id()
    }

    pub fn downcast<T: Topic>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref()
    }
}

impl Debug for AnyTopic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug_fmt(f)
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

        Some(Ref::map(self.0.borrow(), |m| {
            m[&tid][idx].downcast::<T>().unwrap()
        }))
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

    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::PublishTopic(T::id()));
    }
}

pub struct PublisherBuilder<T>(PhantomData<T>);

impl<'c, T: Topic> HandlerFnArgBuilder<'c> for PublisherBuilder<T> {
    type Arg = Publisher<'c, T>;

    fn build(context: &'c Context) -> anyhow::Result<Publisher<'c, T>> {
        Ok(Publisher(context.topics, PhantomData))
    }
}

pub struct Subscriber<'t, T: Topic>(&'t TopicContainer, PhantomData<&'t T>);

impl<'t, T: Topic> Subscriber<'t, T> {
    pub fn iter(&self) -> impl Iterator + '_ {
        (0..).into_iter().map_while(move |idx| self.0.get::<T>(idx))
    }
}

impl<'t, T: Topic> HandlerFnArg for Subscriber<'t, T> {
    type Builder = SubscriberBuilder<T>;

    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::SubscribeTopic(T::id()));
    }
}

pub struct SubscriberBuilder<T>(PhantomData<T>);

impl<'c, T: Topic> HandlerFnArgBuilder<'c> for SubscriberBuilder<T> {
    type Arg = Subscriber<'c, T>;

    fn build(context: &'c Context) -> anyhow::Result<Subscriber<'c, T>> {
        Ok(Subscriber(context.topics, PhantomData))
    }
}
