// A click on the settings page: change that option (or reset them all) and keep it.
fn server_flags_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use horizon_client::multiplayer::server_flags as m;
    if mod_id != m::ID {
        return;
    }
    let mut settings = {
        let game = &*game;
        m::settings(|key| game.native_setting(m::ID, key))
    };
    let stored = settings.click(button);
    m::set_settings(settings);
    for (key, value) in stored {
        game.native_set_setting(m::ID, key, &value);
    }
}
