use std::any::Any;

#[derive(Default)]
pub struct Context {
    #[cfg(not(feature = "async"))]
    inner: Vec<Option<Box<dyn Any>>>,
    #[cfg(feature = "async")]
    inner: Vec<Option<Box<dyn Any + Send>>>,
}

impl Context {
    #[cfg(not(feature = "async"))]
    pub fn insert<T>(&mut self, param: T)
        where T: 'static {
        self.inner.push(Some(Box::new(param)));
    }

    #[cfg(feature = "async")]
    pub fn insert<T>(&mut self, param: T)
        where T: 'static + Send {
        self.inner.push(Some(Box::new(param)));
    }

    pub fn get<T>(&self) -> Option<&T>
        where T: 'static {
        // iterate from end to start
        for item in self.inner.iter().rev() {
            if let Some(item) = item {
                if let Some(data) = item.downcast_ref::<T>() {
                    return Some(data);
                }
            }
        }
        None
    }

    pub fn get_mut<T>(&mut self) -> Option<&mut T>
        where T: 'static {
        // iterate from end to start
        for item in self.inner.iter_mut().rev() {
            if let Some(item) = item {
                if let Some(data) = item.downcast_mut::<T>() {
                    return Some(data);
                }
            }
        }
        None
    }

    pub fn take<T>(&mut self) -> Option<Box<T>>
        where T: 'static {
        // iterate from end to start
        for item in self.inner.iter_mut().rev() {
            if let Some(inner) = item {
                if inner.is::<T>() {
                    let data = item.take().unwrap();
                    return Some(data.downcast().unwrap());
                }
            }
        }
        None
    }
}
