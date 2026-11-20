//! Server Flags on Horizon's mod API. The game patch in `src/mixins/draw.rs`
//! (see horizon-mod.toml) draws the flags on the server list; the hooks here
//! keep the settings.
use mod_api::{
    self, ClientHost, ClientTickPostEvent, Hooks, Registrar, RegistrationContext,
    SettingChangedEvent, SettingSpec,
};

/// The mod's logic, also read by the game patch.
pub mod flags;

use flags::{OPTS, Settings};

/// The settings as the page keeps them.
fn read(host: &dyn ClientHost) -> Settings {
    Settings::from_choices(|key| host.setting_choice(key))
}

#[derive(Default)]
struct ServerFlags;

#[mod_api::horizon_mod]
impl Hooks for ServerFlags {
    /// The settings page: one choice per option.
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let defaults = Settings::default();
        for (i, opt) in OPTS.iter().enumerate() {
            let options = opt.choices.iter().map(|(v, _)| flags::choice_key(opt.key, *v));
            let _ = r.register_setting(SettingSpec::choice(
                opt.key,
                flags::label_key(opt.key),
                options,
                defaults.choice_index(i),
            ));
        }
    }

    /// The player changed an option on the page.
    fn setting_changed(&self, host: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        flags::set_settings(read(&*host));
    }

    /// Each tick, the title screen too: the settings for the server list.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        flags::set_settings(read(&*host));
    }
}
