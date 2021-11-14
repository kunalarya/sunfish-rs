use copy_from::CopyFrom;

#[test]
fn struct_copy_from() {
    #[derive(CopyFrom, Debug, Eq, PartialEq)]
    struct SubNamedStruct {
        mox: isize,
    }

    #[derive(CopyFrom, Debug, Eq, PartialEq)]
    struct SubPosStruct(usize, i32);

    #[derive(CopyFrom, Debug, Eq, PartialEq)]
    struct Test {
        foo: u8,
        sub: SubNamedStruct,
        pos: SubPosStruct,
        other: isize,
    }

    let a = Test {
        foo: 1,
        sub: SubNamedStruct { mox: 100 },
        pos: SubPosStruct(10, 20),
        other: 30,
    };
    let mut b = Test {
        foo: 2,
        sub: SubNamedStruct { mox: 200 },
        pos: SubPosStruct(500, 600),
        other: -99,
    };
    b.copy_from(&a);

    assert_eq!(a, b);
}

#[test]
fn struct_copy_from_with_float() {
    #[derive(CopyFrom, Debug)]
    struct SubNamedStruct {
        mox: f32,
    }

    #[derive(CopyFrom, Debug)]
    struct Test {
        sub: SubNamedStruct,
    }

    let a = Test {
        sub: SubNamedStruct { mox: 100.0 },
    };
    let mut b = Test {
        sub: SubNamedStruct { mox: 200.0 },
    };
    b.copy_from(&a);

    assert_eq!(a.sub.mox, b.sub.mox);
}
