// Each frame (the title screen too): whether the mod is on in Options > Mods,
// and its settings, for the server list.
fn server_flags_frame(game: &mut crate::game::Game<'_>) {
    use horizon_client::multiplayer::server_flags as m;
    if !game.native_window() {
        return;
    }
    m::set_on(game.native_on(m::ID));
    let game = &*game;
    m::settings(|key| game.native_setting(m::ID, key));
}
