use lazy_static::lazy_static;

lazy_static! {
    // Database
    pub static ref PRIMARY_DATABASE_URL: String =
        std::env::var("PRIMARY_DATABASE_URL").unwrap_or_else(|_|
            std::env::var("DATABASE_URL").expect("PRIMARY_DATABASE_URL or DATABASE_URL required")
        );
    pub static ref REPLICA_DATABASE_URL: String =
        std::env::var("REPLICA_DATABASE_URL").unwrap_or_else(|_| PRIMARY_DATABASE_URL.clone());
    pub static ref REDIS_URL: String =
        std::env::var("REDIS_URL").expect("REDIS_URL required");

    // RPC
    pub static ref MAIN_RPC_URL: String =
        std::env::var("MAIN_RPC_URL").expect("MAIN_RPC_URL required");
    pub static ref SUB_RPC_URL_1: String =
        std::env::var("SUB_RPC_URL_1").unwrap_or_default();
    pub static ref SUB_RPC_URL_2: String =
        std::env::var("SUB_RPC_URL_2").unwrap_or_default();
    pub static ref RPC_TIMEOUT_MS: u64 =
        std::env::var("RPC_TIMEOUT_MS").unwrap_or_else(|_| "30000".to_string())
            .parse().unwrap_or(30000);

    // Contract Addresses
    pub static ref IDO_CONTRACT: String =
        std::env::var("IDO_CONTRACT").expect("IDO_CONTRACT required");
    pub static ref LP_MANAGER_CONTRACT: String =
        std::env::var("LP_MANAGER_CONTRACT").expect("LP_MANAGER_CONTRACT required");
    pub static ref POOL_MANAGER_CONTRACT: String =
        std::env::var("POOL_MANAGER_CONTRACT").expect("POOL_MANAGER_CONTRACT required");
    pub static ref SWAP_FEE_HOOK: String =
        std::env::var("SWAP_FEE_HOOK").expect("SWAP_FEE_HOOK required");
    pub static ref USDC_ADDRESS: String =
        std::env::var("USDC_ADDRESS").expect("USDC_ADDRESS required");

    // Chain
    pub static ref CHAIN_ID: u64 =
        std::env::var("CHAIN_ID").unwrap_or_else(|_| "43114".to_string())
            .parse().expect("CHAIN_ID must be a number");

    // Connection pool
    pub static ref PG_PRIMARY_MAX_CONNECTIONS: u32 =
        std::env::var("PG_PRIMARY_MAX_CONNECTIONS").unwrap_or_else(|_| "50".to_string())
            .parse().unwrap_or(50);
    pub static ref PG_PRIMARY_MIN_CONNECTIONS: u32 =
        std::env::var("PG_PRIMARY_MIN_CONNECTIONS").unwrap_or_else(|_| "5".to_string())
            .parse().unwrap_or(5);
    pub static ref PG_REPLICA_MAX_CONNECTIONS: u32 =
        std::env::var("PG_REPLICA_MAX_CONNECTIONS").unwrap_or_else(|_| "200".to_string())
            .parse().unwrap_or(200);
    pub static ref PG_REPLICA_MIN_CONNECTIONS: u32 =
        std::env::var("PG_REPLICA_MIN_CONNECTIONS").unwrap_or_else(|_| "10".to_string())
            .parse().unwrap_or(10);

    // R2 Storage
    pub static ref R2_ACCOUNT_ID: String =
        std::env::var("R2_ACCOUNT_ID").unwrap_or_default();
    pub static ref R2_ACCESS_KEY_ID: String =
        std::env::var("R2_ACCESS_KEY_ID").unwrap_or_default();
    pub static ref R2_SECRET_ACCESS_KEY: String =
        std::env::var("R2_SECRET_ACCESS_KEY").unwrap_or_default();
    pub static ref R2_IMAGE_BUCKET: String =
        std::env::var("R2_IMAGE_BUCKET").unwrap_or_else(|_| "openlaunch-image".to_string());
    pub static ref R2_METADATA_BUCKET: String =
        std::env::var("R2_METADATA_BUCKET").unwrap_or_else(|_| "openlaunch-metadata".to_string());
    pub static ref R2_IMAGE_PUBLIC_URL: String =
        std::env::var("R2_IMAGE_PUBLIC_URL").unwrap_or_default();
    pub static ref R2_METADATA_PUBLIC_URL: String =
        std::env::var("R2_METADATA_PUBLIC_URL").unwrap_or_default();
}
