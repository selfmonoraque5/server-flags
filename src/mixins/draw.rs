// The title screens have drawn (`draw_title_screen_with_post_hook`, before the
// mods' `screen_render_post`): on the server list, each saved server's flag
// where its row is (as `multiplayer.rs` lays the rows out), a grey "?" for a
// server whose location can't be shown, and the reason or the country's name
// when the pointer is over it. Rows scrolled out of the list are left out.
fn server_flags_draw(game: &Game<'_>, batch: &mut HudBatch) {
    use mod_server_flags::flags as m;
    let Some(title) = game.title.as_ref() else {
        return;
    };
    let horizon_client::title::Mode::Multiplayer(horizon_client::multiplayer::Screen::List {
        servers,
        lan,
        scroll,
        ..
    }) = &title.mode
    else {
        return;
    };
    let atlas = &game.gui.atlas;
    let settings = m::current();
    let size = (batch.width.ceil(), batch.height.ceil());
    let (top, bottom, left) = m::list_area(size.0, size.1);
    let mut scroll = *scroll;
    scroll.clamp(top, bottom, m::content_height(servers.len(), lan.games().len()));
    // Leave the player count and the status icon their room.
    let room = left + 2.0 + m::ROW_WIDTH - 4.0 - 15.0 - 50.0;
    let mut hovered = None;
    for (k, server) in servers.iter().enumerate() {
        let y = top + 2.0 - scroll.amount.trunc() + k as f32 * m::ROW_HEIGHT;
        let (cx, cy) = (left + 2.0, y + 2.0);
        let Some(place) = m::place(&server.ip) else {
            continue;
        };
        let name_w = HudBatch::text_width(atlas, &server.name);
        let [fx, fy] = m::flag_at(settings.get(m::o::PLACE), cx, cy, name_w, room);
        if fy - 1.0 < top || fy + m::FLAG_H + 1.0 > bottom {
            continue;
        }
        // A badge: a dark box with `text` in it.
        let mut badge = |text: &str, color: u32| {
            let tw = HudBatch::text_width(atlas, text);
            batch.fill(atlas, fx - 1.0, fy - 1.0, tw + 3.0, m::FLAG_H + 2.0, m::rgba(0, 0, 0, 170));
            batch.text(atlas, text, fx + 0.5, fy, color, false);
            tw + 1.0
        };
        let width = match &place {
            m::Place::Country(code) => match m::flag(code) {
                Some(cells) => {
                    batch.fill(atlas, fx - 1.0, fy - 1.0, m::FLAG_W + 2.0, m::FLAG_H + 2.0, m::rgba(0, 0, 0, 160));
                    for (x, y, w, h, [r, g, b]) in cells {
                        batch.fill(
                            atlas,
                            fx + f32::from(x),
                            fy + f32::from(y),
                            f32::from(w),
                            f32::from(h),
                            m::rgba(r, g, b, 255),
                        );
                    }
                    m::FLAG_W
                }
                None if settings.on(m::o::CODES) => badge(code, u32::MAX),
                None => continue,
            },
            p if m::unknown(p) && settings.on(m::o::UNKNOWN) => badge("?", m::rgba(170, 170, 170, 255)),
            _ => continue,
        };
        let (px, py) = game.cursor;
        if px >= fx - 1.0 && px < fx + width + 1.0 && py >= fy - 1.0 && py < fy + m::FLAG_H + 1.0 {
            hovered = m::tooltip(&place);
        }
    }
    if settings.on(m::o::TOOLTIP)
        && let Some(line) = hovered
    {
        horizon_client_render::ui::tooltip(batch, atlas, &[horizon_client_render::text::Text::literal(line.as_str())], game.cursor);
    }
}
