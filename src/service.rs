use std::sync::mpsc::channel;

use chainhook_sdk::{
    observer::{ObserverEvent, ObserverCommand, start_event_observer, ObserverSidecar},
    types::{BlockIdentifier, BitcoinChainEvent},
    utils::Context,
};
use crate::config::Config;

pub fn start_service(config: &Config, ctx: &Context) -> Result<(), String> {
    let (observer_cmd_tx, observer_cmd_rx) = channel();
    let (observer_event_tx, observer_event_rx) = crossbeam_channel::unbounded();


    let (block_pre_processor_in_tx, block_pre_processor_in_rx) = crossbeam_channel::unbounded();
    let (block_pre_processor_out_tx, block_pre_processor_out_rx) = crossbeam_channel::unbounded();

    let observer_sidecar = ObserverSidecar {
        bitcoin_blocks_mutator: Some((block_pre_processor_in_tx, block_pre_processor_out_rx)),
        bitcoin_chain_event_notifier: None,
    };

    // let (ordinal_indexer_cmd_tx, ordinal_indexer_cmd_rx) = channel();

    // Start chainhook event observer
    let event_observer_config = config.event_observer.clone();
    let context = if config.event_observer.display_logs {
        ctx.clone()
    } else {
        Context::empty()
    };
    let observer_cmd_tx_moved = observer_cmd_tx.clone();
    
    let _ = std::thread::spawn(move || {
        start_event_observer(
            event_observer_config,
            observer_cmd_tx_moved,
            observer_cmd_rx,
            Some(observer_event_tx),
            Some(observer_sidecar),
            None,
            context,
        )
        .expect("unable to start Stacks chain observer");
    });

    let context_cloned = ctx.clone();
    let config_cloned = config.clone();

    loop {
        let event = match observer_event_rx.recv() {
            Ok(cmd) => cmd,
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error: broken channel {}",
                    e.to_string()
                );
                break;
            }
        };

        match event {
            ObserverEvent::BitcoinChainEvent((BitcoinChainEvent::ChainUpdatedWithBlocks(blocks), _)) => {
                for block in blocks


            }
            ObserverEvent::Terminate => {
            }
            _ => {}
        }
    }
    Ok(())
}
