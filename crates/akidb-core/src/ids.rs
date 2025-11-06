use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a new identifier using UUID v7.
            #[must_use]
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Creates an identifier from an existing UUID.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Returns the inner UUID value.
            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Returns the raw 16-byte representation suitable for SQLite blobs.
            #[must_use]
            pub const fn to_bytes(self) -> [u8; 16] {
                self.0.into_bytes()
            }

            /// Creates an identifier from raw bytes.
            ///
            /// # Errors
            ///
            /// Returns `uuid::Error` when the bytes do not form a valid UUID.
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, uuid::Error> {
                Uuid::from_slice(bytes).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

define_id!(TenantId, "Unique identifier for a tenant.");
define_id!(
    DatabaseId,
    "Unique identifier for a database namespace within a tenant."
);
define_id!(
    CollectionId,
    "Unique identifier for a collection within a database."
);
define_id!(
    UserId,
    "Unique identifier for an authenticated user within a tenant."
);
define_id!(
    AuditLogId,
    "Unique identifier for an audit log entry."
);
define_id!(
    DocumentId,
    "Unique identifier for a vector document within a collection."
);
