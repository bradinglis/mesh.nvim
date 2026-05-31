use std::ops::FromResidual;

#[derive(Debug)]
pub struct Results<T, E> {
    pub oks: Vec<T>,
    pub errs: Vec<E>,
}

impl<T, E> From<Vec<std::result::Result<T, E>>> for Results<T, E> {
    fn from(value: Vec<std::result::Result<T, E>>) -> Self {
        Self::from_iter(value)
    }
}

impl<T, E> FromIterator<std::result::Result<T, E>> for Results<T, E> {
    fn from_iter<I: IntoIterator<Item = std::result::Result<T, E>>>(iter: I) -> Self {
        let mut res = Self {
            oks: vec![],
            errs: vec![],
        };

        for i in iter {
            match i {
                Ok(ok) => res.oks.push(ok),
                Err(err) => res.errs.push(err),
            }
        }
        res
    }
}

impl<T, E, F> FromResidual<Result<std::convert::Infallible, E>> for Results<T, F>
where
    F: From<E>,
{
    fn from_residual(residual: Result<std::convert::Infallible, E>) -> Self {
        match residual {
            Err(e) => Results {
                oks: vec![],
                errs: vec![e.into()],
            },
        }
    }
}
