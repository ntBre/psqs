pub mod atom;
pub mod program;
pub mod queue;

mod dump;

/// from [StackOverflow](https://stackoverflow.com/a/45145246)
#[macro_export]
macro_rules! string {
    // match a list of expressions separated by comma:
    ($($str:expr),*) => ({
        // create a Vec with this list of expressions,
        // calling String::from on each:
        vec![$(String::from($str),)*] as Vec<String>
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
