use anni_repo::library::{parts_to_date, album_info, disc_info};

#[test]
fn test_parts_to_date() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(parts_to_date("2020", "01", "02")?.to_string(), "2020-01-02");
    assert_eq!(parts_to_date("20", "01", "02")?.to_string(), "2020-01-02");
    assert_eq!(parts_to_date("99", "01", "02")?.to_string(), "1999-01-02");
    Ok(())
}

#[test]
fn test_album_info() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(album_info("[200102][CATA-001] TITLE")?,
               (parts_to_date("2020", "01", "02")?, "CATA-001".to_owned(), "TITLE".to_owned()));

    Ok(())
}

#[test]
fn test_disc_info() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(disc_info("[CATA-001] TITLE [Disc 1]")?,
               ("CATA-001".to_owned(), "TITLE".to_owned(), 1));
    Ok(())
}