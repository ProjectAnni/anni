use anni_common::inherit::InheritableValue;

#[test]
fn test_inherit() {
    let o = InheritableValue::own(1);
    let mut i = InheritableValue::default();
    i.inherit_from(&o);
    assert_eq!(i.as_ref(), &1);

    let mut i = InheritableValue::default();
    i.inherit_from_owned(&1);
    assert_eq!(i.as_ref(), &1);
}

#[test]
#[should_panic]
fn test_double_inherit() {
    let mut i = InheritableValue::default();
    i.inherit_from_owned(&1);
    i.inherit_from_owned(&1);
}
