use chainhook_sdk::observer::EventObserverConfigOverrides;

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigFile {
    pub event_observer: Option<EventObserverConfigOverrides>,
}
