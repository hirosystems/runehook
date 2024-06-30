use chainhook_sdk::utils::Context;
use index_cache::IndexCache;
use tokio_postgres::Client;

use crate::config::Config;

use super::pg_get_max_rune_number;

pub mod db_cache;
pub mod index_cache;
pub mod transaction_cache;
pub mod transaction_location;

/// Creates a blank index cache pointing to the correct next rune number to etch.
pub async fn new_index_cache(config: &Config, pg_client: &mut Client, ctx: &Context) -> IndexCache {
    IndexCache::new(
        config.get_bitcoin_network(),
        config.resources.lru_cache_size,
        pg_get_max_rune_number(pg_client, ctx).await,
    )
}
