use std::future::Future;

pub type SttResult<T> = Result<(), T>;

pub trait AsyncExecute {
    type Output;

    fn execute_async(&self) -> impl Future<Output = Self::Output>;
}

pub trait AsyncSend<M> {
    fn send(&self, message: M) -> impl Future<Output = ()>;
}

pub trait Endpoint {
    type Config;
    type Output;

    fn init(config: Self::Config) -> impl Future<Output = Option<Self::Output>>;
}

pub trait Channel {
    type Message;
    type Writer;

    fn open(&mut self, capacity: usize) -> impl Future<Output = ()>;
    fn close(&mut self) -> impl Future<Output = ()>;
}