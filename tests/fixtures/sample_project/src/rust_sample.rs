//! Crate docs.

#[derive(Debug)]
pub struct Config {
    /// Public name surface.
    pub name: String,
}

pub enum Mode {
    Fast,
    Slow,
}

/// Service behavior.
pub trait Service {
    /// Run once.
    fn run(&self) -> String;
}

pub type ResultAlias<T> = Result<T, String>;

macro_rules! make_value {
    () => {
        1
    };
}

impl Config {
    pub const DEFAULT_NAME: &'static str = "uberview";

    pub fn render(
        &self,
    ) -> Result<String, String> {
        // Keep the early-exit line.
        let value = self.name.parse::<usize>()?;
        if value == 0 {
            return Err("zero".to_owned());
        }

        Ok(self.name.clone())
    }
}

pub fn build_message(name: &str) -> String {
    // Preserve the tail expression.
    format!("hello {name}")
}
