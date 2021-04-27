//! The Apollo framework is a toolkit for building stable and robust server software.
//!
//! **Apollo** was extracted out of [Jupiter](https://github.com/scireum/jupiter) to make its
//! core parse usable by other libraries or applications.
//!
//! Apollo provides a small **dependency injection helper** called *Platform** along with some
//! tooling to setup logging, format messages and to react on signals. This is commonly required
//! to properly run inside a Docker container.
#![deny(
missing_docs,
trivial_casts,
trivial_numeric_casts,
unused_extern_crates,
unused_import_braces,
unused_results
)]

use simplelog::{ConfigBuilder, LevelFilter, SimpleLogger};
use std::sync::Once;

pub mod average;
pub mod fmt;
pub mod platform;
pub mod signals;

/// Contains the version of the Apollo framework.
pub const APOLLO_VERSION: &str = "DEVELOPMENT-SNAPSHOT";

/// Contains the git commit hash of the Apollo build being used.
pub const APOLLO_REVISION: &str = "NO-REVISION";

/// Initializes the logging system.
///
/// This uses the simple logger crate with a logging format which is compatible with docker
/// environments and quite well readable by common tools.
pub fn init_logging() {
    static INIT_LOGGING: Once = Once::new();
    static DATE_FORMAT: &str = "[%Y-%m-%dT%H:%M:%S%.3f]";

    // We need to do this as otherwise the integration tests might crash as the logging system
    // is initialized several times...
    INIT_LOGGING.call_once(|| {
        if let Err(error) = SimpleLogger::init(
            LevelFilter::Debug,
            ConfigBuilder::new()
                .set_time_format_str(DATE_FORMAT)
                .set_thread_level(LevelFilter::Trace)
                .set_target_level(LevelFilter::Error)
                .set_location_level(LevelFilter::Trace)
                .build(),
        ) {
            panic!("Failed to initialize logging system: {}", error);
        }
    });
}