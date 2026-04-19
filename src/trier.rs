pub struct Tryer<U> {
    inner: Option<U>,
}

impl<U> Tryer<U>
where
    U: Clone,
{
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn or_try<F, K, J>(self, f: F) -> Tryer<U>
    where
        F: FnOnce() -> J,
        J: Into<Tryer<K>>,
        K: Into<U>,
    {
        Tryer {
            inner: match self.inner {
                Some(x) => Some(x),
                None => f().into().inner.map(|x| x.into()),
            },
        }
    }

    pub fn unwrap_or(self, default: U) -> U {
        self.inner.unwrap_or(default)
    }
    pub fn unwrap_or_lazy<F>(self, default: F) -> U
    where
        F: FnOnce() -> U,
    {
        self.inner.unwrap_or(default())
    }
}

impl<T> From<Option<T>> for Tryer<T> {
    fn from(val: Option<T>) -> Self {
        Tryer { inner: val }
    }
}

impl<T, E> From<Result<T, E>> for Tryer<T> {
    fn from(val: Result<T, E>) -> Self {
        Tryer { inner: val.ok() }
    }
}
