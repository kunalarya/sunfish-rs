/// All VST parameters should support consistently ordered enumeration. To do so, we expect them
/// to implement the Enumerable trait.
pub trait Enumerable<T> {
    fn enumerate() -> Vec<T>;
}
