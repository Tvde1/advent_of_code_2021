use std::mem::{swap, MaybeUninit};

use super::*;

pub trait ParserMultiExt: Sized + Parser {
    /// Repeatedly applies the parser, interspersing applications of `separator`.
    /// Fails if parser cannot be applied at least once.
    fn sep_by<S>(self, separator: S) -> SepBy<Self, S>
    where
        S: Parser,
    {
        SepBy {
            parser: self,
            separator,
        }
    }

    /// Repeatedly applies the parser, repeatedly invoking `func` with the
    /// output value, updating the accumulator which starts out as `initial`.
    fn fold<A, F>(self, initial: A, func: F) -> Fold<Self, A, F>
    where
        A: Clone,
        F: Fn(A, Self::Output) -> A,
    {
        Fold {
            parser: self,
            initial,
            func,
        }
    }

    /// Repeatedly applies the parser, repeatedly invoking `func` with the
    /// output value, updating the accumulator which starts out as `initial`.
    fn fold_mut<A, F>(self, initial: A, func: F) -> FoldMut<Self, A, F>
    where
        A: Clone,
        F: Fn(&mut A, Self::Output),
    {
        FoldMut {
            parser: self,
            initial,
            func,
        }
    }

    /// Repeatedly applies the parser, until failure, returning the last
    /// successful output, or an error if it fails to apply even once.
    fn repeat(self) -> Repeat<Self> {
        Repeat { parser: self }
    }

    fn many_n<const N: usize>(self) -> Many<Self, N> {
        Many { parser: self }
    }
}

impl<P: Parser> ParserMultiExt for P {}

#[derive(Debug, Clone, Copy)]
pub struct SepBy<P, S> {
    parser: P,
    separator: S,
}

#[derive(Debug, Clone, Copy)]
pub struct Fold<P, A, F> {
    parser: P,
    initial: A,
    func: F,
}

#[derive(Debug, Clone, Copy)]
pub struct FoldMut<P, A, F> {
    parser: P,
    initial: A,
    func: F,
}

#[derive(Debug, Clone, Copy)]
pub struct Repeat<P> {
    parser: P,
}

#[derive(Debug, Clone, Copy)]
pub struct Many<P, const N: usize> {
    parser: P,
}

impl<P, S> Parser for SepBy<P, S>
where
    P: Parser,
    S: Parser,
{
    type Output = Vec<P::Output>;

    fn parse<'s>(&self, input: &'s str) -> ParseResult<'s, Self::Output> {
        let (element, mut remainder) = self.parser.parse(input)?;
        let mut elements = Vec::new();
        elements.push(element);
        loop {
            let after_sep = match self.separator.parse(remainder) {
                Ok((_, after_sep)) => after_sep,
                Err(_) => return Ok((elements, remainder)),
            };
            match self.parser.parse(after_sep) {
                Ok((element, after_value)) => {
                    remainder = after_value;
                    elements.push(element);
                }
                Err(_) => return Ok((elements, remainder)),
            };
        }
    }
}

impl<P, A, F> Parser for Fold<P, A, F>
where
    P: Parser,
    A: Clone,
    F: Fn(A, P::Output) -> A,
{
    type Output = A;

    fn parse<'s>(&self, input: &'s str) -> ParseResult<'s, Self::Output> {
        let mut accumulator = self.initial.clone();
        let mut remainder = input;
        while let Ok((value, new_remainder)) = self.parser.parse(remainder) {
            accumulator = (self.func)(accumulator, value);
            remainder = new_remainder;
        }
        Ok((accumulator, remainder))
    }
}

impl<P, A, F> Parser for FoldMut<P, A, F>
where
    P: Parser,
    A: Clone,
    F: Fn(&mut A, P::Output),
{
    type Output = A;

    fn parse<'s>(&self, input: &'s str) -> ParseResult<'s, Self::Output> {
        let mut accumulator = self.initial.clone();
        let mut remainder = input;
        while let Ok((value, new_remainder)) = self.parser.parse(remainder) {
            (self.func)(&mut accumulator, value);
            remainder = new_remainder;
        }
        Ok((accumulator, remainder))
    }
}

impl<P> Parser for Repeat<P>
where
    P: Parser,
{
    type Output = P::Output;

    fn parse<'s>(&self, input: &'s str) -> ParseResult<'s, Self::Output> {
        let (mut last_value, mut remainder) = match self.parser.parse(input) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        while let Ok((value, new_remainder)) = self.parser.parse(remainder) {
            last_value = value;
            remainder = new_remainder;
        }
        Ok((last_value, remainder))
    }
}

impl<P: Parser, const N: usize> Parser for Many<P, N> {
    type Output = [P::Output; N];

    fn parse<'s>(&self, input: &'s str) -> ParseResult<'s, Self::Output> {
        struct PartiallyInit<T, const N: usize> {
            memory: [MaybeUninit<T>; N],
            count: usize,
        }

        impl<T, const N: usize> Drop for PartiallyInit<T, N> {
            fn drop(&mut self) {
                for i in (0..self.count).rev() {
                    unsafe {
                        self.memory[i].assume_init_drop();
                    }
                }
            }
        }

        let mut partially_init = PartiallyInit::<P::Output, N> {
            memory: MaybeUninit::uninit_array(),
            count: 0,
        };

        let mut remainder = input;
        while partially_init.count < N {
            let (value, new_remainder) = self.parser.parse(remainder)?;
            remainder = new_remainder;
            partially_init.memory[partially_init.count].write(value);
            partially_init.count += 1;
        }

        let result = unsafe {
            let mut memory = MaybeUninit::uninit_array();
            swap(&mut memory, &mut partially_init.memory);
            MaybeUninit::array_assume_init(memory)
        };
        Ok((result, remainder))
    }
}
