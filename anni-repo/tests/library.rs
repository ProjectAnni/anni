use anni_repo::prelude::*;

#[test]
fn test_parts_to_date() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        AnniDate::from_parts("2020", "01", "02").to_string(),
        "2020-01-02"
    );
    assert_eq!(
        AnniDate::from_parts("20", "01", "02").to_string(),
        "2020-01-02"
    );
    assert_eq!(
        AnniDate::from_parts("99", "01", "02").to_string(),
        "1999-01-02"
    );
    Ok(())
}
