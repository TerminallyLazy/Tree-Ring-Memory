pub mod adapters;
pub mod audit;
pub mod export_import;
pub mod integrations;
pub mod lifecycle;
pub mod recall;
pub mod remember;

pub type ActionResult<T> = Result<T, String>;
