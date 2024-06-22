use chainhook_sdk::utils::Context;
use index_cache::IndexCache;
use tokio_postgres::Client;

use super::pg_get_max_rune_number;

pub mod db_cache;
pub mod index_cache;
pub mod transaction_cache;

/// Creates a blank index cache pointing to the correct next rune number to etch.
pub async fn new_index_cache(pg_client: &mut Client, ctx: &Context) -> IndexCache {
    IndexCache::new(
        bitcoin::Network::Bitcoin,
        5000,
        pg_get_max_rune_number(pg_client, ctx).await,
    )
}
