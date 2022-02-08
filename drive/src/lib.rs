pub mod common;
pub mod contract;
pub mod drive;
pub mod query;
mod identity;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
