pub struct Tryer<U> {
    inner: Option<U>,
}

impl<U> Default for Tryer<U> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<U> Tryer<U> {
    pub fn new() -> Self {
        Self::default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn or_try_uses_first_success() {
        let value = Tryer::new()
            .or_try(|| Some(7))
            .or_try(|| Some(9))
            .unwrap_or(0);
        assert_eq!(value, 7);
    }

    #[test]
    fn falls_back_to_default() {
        let value = Tryer::<i32>::new().unwrap_or(42);
        assert_eq!(value, 42);
    }
}
