pub use copy_from_derive::CopyFrom;

pub trait CopyFrom {
    fn copy_from(&mut self, other: &Self);
}

impl<T: Copy> CopyFrom for T {
    fn copy_from(&mut self, other: &T) {
        *self = *other;
    }
}

#[cfg(test)]
mod tests {
    use super::CopyFrom;

    #[allow(clippy::float_cmp)]
    #[test]
    fn basic_copying_ints() {
        macro_rules! check {
            ($typ: ty) => {{
                let x: $typ = <$typ>::default();
                let mut y: $typ = <$typ>::default() + (1 as $typ);
                y.copy_from(&x);
                assert_eq!(x, y);
            }};
        }
        check!(u8);
        check!(u16);
        check!(u32);
        check!(usize);
        check!(i8);
        check!(i16);
        check!(i32);
        check!(isize);
        check!(f32);
        check!(f64);
    }

    #[test]
    fn basic_copying_bools() {
        let x = true;
        let mut y = false;
        y.copy_from(&x);
        assert_eq!(x, y);
    }
}
