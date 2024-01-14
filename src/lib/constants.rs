use surrealdb::sql::Thing;

pub struct Constants {}

impl Constants {
    pub fn system_thing() -> Thing {
        Thing {
            tb: "person".into(),
            id: "01HM3BK88HHTMGMXXEX5V8DZK5".into(),
        }
    }
}
