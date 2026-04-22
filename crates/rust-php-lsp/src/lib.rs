pub mod analyzers;
pub mod context;
pub mod index;
pub mod query;
mod server;
pub mod types;

pub use rust_php_foundation::php;
pub use rust_php_foundation::project;

pub mod overrides {
    pub use rust_php_foundation::overrides::FileOverrides;
}

pub mod lsp {
    pub use crate::{context, index, query};

    pub mod overrides {
        pub use rust_php_foundation::overrides::FileOverrides;
    }
}

pub use server::run_stdio;
