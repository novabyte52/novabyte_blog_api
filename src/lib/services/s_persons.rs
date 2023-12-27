use ulid::Ulid;

use crate::{models::person::Person, repos::r_posts};

pub async fn create_person() -> Person {
    println!("s: create person");
    return r_persons::insert_person().await;
}

pub async fn get_person(person_id: Ulid) -> Person {
    println!("s: get person");

    return r_persons::select_person(person_id).await;
}

pub async fn get_persons() -> Vec<Person> {
    println!("s: get persons");
    let foo = r_persons::select_persons().await;
    println!("s: {:#?}", foo);
    foo
}
