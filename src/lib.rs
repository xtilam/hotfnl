

pub use hot_macro::*;
#[cfg(not(feature = "prod"))]
mod hotreload;
#[cfg(not(feature = "prod"))]
pub use hotreload::*;

#[cfg(feature = "prod")]
mod prod;
#[cfg(feature = "prod")]
pub use prod::*;
