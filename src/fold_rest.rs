use crate::Result;
use crate::parse::Lines;

pub enum IterState {
    Continue,
    Break,
}

/// Apply `f` to the remaining lines until it asks to break, returning the
/// unconsumed iterator.
pub fn for_each_rest<F>(mut iter: Lines, mut f: F) -> Result<Lines>
where
    F: FnMut(Result<String>) -> Result<IterState>,
{
    for line in iter.by_ref() {
        if let IterState::Break = f(line)? {
            break;
        }
    }
    Ok(iter)
}
