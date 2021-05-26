use anni_repo::category::{Category, CategoryType};
use std::str::FromStr;

#[test]
fn test_category_deserialize() {
    let category = Category::from_str(r#"
[category]
name = "THE IDOLM@STER Shiny Colors"
type = "AAAA"
albums = [
  "LACM-14781",
  "LACM-14861",
  "LACM-14982",
  "LACM-14906",
  "LACM-14965",
  "LACM-14782",
  "LACM-14862",
  "LACM-14983",
  "LACM-14783",
  "LACM-14863",
  "LACM-14984",
  "LACM-14784",
  "LACM-14864",
  "LACM-14985",
  "LACM-14785",
  "LACM-14865",
  "LACM-14986",
  "LACM-14866",
  "LACM-14987",
  "LACM-24005"
]

[[subcategory]]
name = "シャイニーカラーズ"
albums = [
  "LACM-14781",
  "LACM-14861",
  "LACM-14982",
  "LACM-14906",
  "LACM-14965"
]

[[subcategory]]
name = "イルミネーションスターズ"
albums = [
  "LACM-14782",
  "LACM-14862",
  "LACM-14983"
]

[[subcategory]]
name = "アンティーカ"
albums = [
  "LACM-14783",
  "LACM-14863",
  "LACM-14984"
]

[[subcategory]]
name = "放課後クライマックスガールズ"
albums = [
  "LACM-14784",
  "LACM-14864",
  "LACM-14985"
]

[[subcategory]]
name = "アルストロメリア"
albums = [
  "LACM-14785",
  "LACM-14865",
  "LACM-14986"
]

[[subcategory]]
name = "ストレイライト"
albums = [
  "LACM-14866",
  "LACM-14987"
]

[[subcategory]]
name = "ノクチル"
albums = [
  "LACM-24005"
]
"#).expect("failed to parse category");
    assert_eq!(category.info().name(), "THE IDOLM@STER Shiny Colors");
    assert_eq!(category.info().category_type(), CategoryType::Group);
    assert_eq!(category.info().albums().collect::<Vec<_>>(), vec!["LACM-14781", "LACM-14861", "LACM-14982", "LACM-14906", "LACM-14965", "LACM-14782", "LACM-14862", "LACM-14983", "LACM-14783", "LACM-14863", "LACM-14984", "LACM-14784", "LACM-14864", "LACM-14985", "LACM-14785", "LACM-14865", "LACM-14986", "LACM-14866", "LACM-14987", "LACM-24005"]);

    for (i, subcategory) in category.subcategories().enumerate() {
        match i {
            0 => {
                assert_eq!(subcategory.name(), "シャイニーカラーズ");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-14781", "LACM-14861", "LACM-14982", "LACM-14906", "LACM-14965"]);
            }
            1 => {
                assert_eq!(subcategory.name(), "イルミネーションスターズ");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-14782", "LACM-14862", "LACM-14983"]);
            }
            2 => {
                assert_eq!(subcategory.name(), "アンティーカ");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-14783", "LACM-14863", "LACM-14984"]);
            }
            3 => {
                assert_eq!(subcategory.name(), "放課後クライマックスガールズ");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-14784", "LACM-14864", "LACM-14985"]);
            }
            4 => {
                assert_eq!(subcategory.name(), "アルストロメリア");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-14785", "LACM-14865", "LACM-14986"]);
            }
            5 => {
                assert_eq!(subcategory.name(), "ストレイライト");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-14866", "LACM-14987"]);
            }
            6 => {
                assert_eq!(subcategory.name(), "ノクチル");
                assert_eq!(subcategory.albums().collect::<Vec<_>>(), vec!["LACM-24005"]);
            }
            _ => unreachable!()
        }
    }
}