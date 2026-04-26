// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use crate::keychain::FileKeychain;
use crate::keychain::KeychainBackend;
use crate::keychain::OsKeychain;

pub struct CliContext {
    pub keychain: Box<dyn KeychainBackend>,
}

impl CliContext {
    /// Production context — OS keychain.
    pub fn production() -> Self {
        Self { keychain: Box::new(OsKeychain) }
    }

    /// Test context — file-based keychain in a fixed temp directory.
    #[allow(clippy::disallowed_methods)]
    pub fn test_mode() -> Self {
        let dir = std::env::temp_dir().join("gosh_test_keychain");
        Self { keychain: Box::new(FileKeychain::new(dir)) }
    }
}
