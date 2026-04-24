pub mod analyzers {
    pub use rust_php_editor::analyzers::*;
}

pub mod context {
    pub use rust_php_editor::context::*;
}

pub mod index {
    pub use rust_php_editor::index::*;
}

pub mod query {
    pub use rust_php_editor::query::*;
}

mod server;
pub mod types {
    pub use rust_php_editor::types::*;
}

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
