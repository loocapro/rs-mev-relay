/// Get validators route
pub mod get_validators;
mod post_bids;

pub use get_validators::get_validators;
pub use post_bids::{post_bids, PostBidsError};
