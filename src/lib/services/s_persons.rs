use surrealdb::sql::{Id, Thing};

use crate::{
    models::person::{Person, PostPerson},
    repos::r_persons::PersonsRepo,
};

pub async fn create_person(new_person: PostPerson) -> Person {
    println!("s: create person");
    return PersonsRepo::new()
        .await
        .insert_person(
            new_person,
            Thing {
                tb: String::from("person"),
                id: "01HJRVBD6MMBJGWJ7BQV3RANQY".into(),
            },
        )
        .await;
}

pub async fn get_person(person_id: Id) -> Person {
    println!("s: get person");

    match PersonsRepo::new().await.select_person(person_id).await {
        Some(p) => p,
        None => panic!("No person found"),
    }
}

pub async fn get_persons() -> Vec<Person> {
    println!("s: get persons");
    let foo = PersonsRepo::new().await.select_persons().await;
    println!("s: {:#?}", foo);
    foo
}
