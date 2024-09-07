use std::env;

pub fn get_env<T: From<std::string::String>>(env_key: &str) -> T {
    env::var(env_key)
        .expect(format!("cannot find {}", env_key).as_str())
        .into()
}
