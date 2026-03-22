use std::{fs, io};
use crate::Result;
use crate::parse::LinesIter;


pub enum IterState {
    Continue,
    Break,
}

pub trait FoldRest {
    type Item;
    // fn fold_rest<C, F>(self, init: C, f: F) -> Result<(C, Self)>
    // where
    //     Self: Sized,
    //     F: FnMut(C, Self::Item) -> Result<IterState<C>>;

    fn for_each_rest<F>(self, f: F) -> Result<LinesIter>
    where
        F: FnMut(Self::Item) -> Result<IterState>;
}

impl FoldRest for LinesIter {

    type Item = Result<String>;
    fn for_each_rest<F>(mut self, mut f: F) -> Result<LinesIter>
    where
        F: FnMut(Self::Item) -> Result<IterState>,
    {
        for x in self.by_ref() {
            let t = f(x)?;
            match t {
                IterState::Break => {
                    break;
                }
                IterState::Continue => {
                    continue;
                }
            }
        }
        Ok(self)
    }
}
//
// impl<B: std::io::BufRead> FoldRest for std::io::Lines<B> {
//
//     type Item = std::io::Result<String>;
//
//     fn for_each_rest<C, F>(mut self, mut f: F) -> Result<Self>
//     where
//         Self: Iterator<Item = std::io::Result<String>> + Sized,
//         F: FnMut(Self::Item) -> Result<IterState>,
//     {
//         while let Some(x) = self.next() {
//             let t = f(x)?;
//             match t {
//                 IterState::Break => {
//                     break;
//                 }
//                 IterState::Continue => {
//                     continue;
//                 }
//             }
//         }
//         Ok(self)
//     }
// }
