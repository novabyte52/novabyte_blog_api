use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use surrealdb::sql::Id;

use crate::{
    constants::Constants,
    models::person::{Person, SignUpState},
    repos::r_persons::PersonsRepo,
};

pub async fn sign_up(mut sign_up_state: SignUpState) -> Person {
    let argon2 = Argon2::default();

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2
        .hash_password(sign_up_state.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    sign_up_state.pass_hash = Some(password_hash);

    PersonsRepo::new()
        .await
        .insert_person(sign_up_state, Constants::system_thing().clone())
        .await
}

// pub async fn create_person(new_person: PostPerson) -> Person {
//     println!("s: create person");
//     return PersonsRepo::new()
//         .await
//         .insert_person(
//             new_person,
//             Thing {
//                 tb: String::from("person"),
//                 id: "01HJRVBD6MMBJGWJ7BQV3RANQY".into(),
//             },
//         )
//         .await;
// }

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
