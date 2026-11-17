#[derive(Debug)]
pub struct Results<T, E> {
    pub oks: Vec<T>,
    pub errs: Vec<E>,
}

impl<T, E> Default for Results<T, E> {
    fn default() -> Self {
        Self {
            oks: Vec::new(),
            errs: Vec::new(),
        }
    }
}

impl<T, E> From<Vec<std::result::Result<T, E>>> for Results<T, E> {
    fn from(value: Vec<std::result::Result<T, E>>) -> Self {
        value.into_iter().collect()
    }
}

impl<T, E> FromIterator<std::result::Result<T, E>> for Results<T, E> {
    fn from_iter<I: IntoIterator<Item = std::result::Result<T, E>>>(iter: I) -> Self {
        let mut res = Self::default();

        for i in iter {
            match i {
                Ok(ok) => res.oks.push(ok),
                Err(err) => res.errs.push(err),
            }
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_oks_and_errs() {
        let input: Vec<Result<i32, &str>> = vec![Ok(1), Err("boom"), Ok(2)];
        let res = Results::from(input);
        assert_eq!(res.oks, vec![1, 2]);
        assert_eq!(res.errs, vec!["boom"]);
    }
}
