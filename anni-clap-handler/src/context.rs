use std::any::Any;

#[derive(Default)]
pub struct Context {
    inner: Vec<Box<dyn Any>>,
}

impl Context {
    pub fn insert<T>(&mut self, param: T)
        where T: 'static {
        self.inner.push(Box::new(param));
    }

    pub fn get<T>(&self) -> Option<&T>
        where T: 'static {
        // iterate from end to start
        for item in self.inner.iter().rev() {
            if let Some(data) = item.downcast_ref::<T>() {
                return Some(data);
            }
        }
        None
    }
}
