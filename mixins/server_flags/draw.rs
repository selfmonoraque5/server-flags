// `Screen::draw_tooltip`, before its own tooltips: on the server list, each
// saved server's flag where its row is (as `draw` lays the rows out), and the
// country's name when the pointer is over a flag. Rows scrolled out of the
// list are left out.
fn server_flags_draw(screen: &Screen, atlas: &HudAtlas, batch: &mut HudBatch, cursor: (f32, f32)) {
    use horizon_client::multiplayer::server_flags as m;
    let Screen::List {
        servers,
        lan,
        scroll,
        ..
    } = screen
    else {
        return;
    };
    if !m::on() {
        return;
    }
    let settings = m::current();
    let size = (batch.width.ceil(), batch.height.ceil());
    let (top, bottom, left) = list_area(size.0, size.1);
    let mut scroll = *scroll;
    scroll.clamp(top, bottom, content_height(servers.len(), lan.games().len()));
    // Leave the player count and the status icon their room.
    let room = left + 2.0 + ROW_WIDTH - 4.0 - 15.0 - 50.0;
    let mut hovered = None;
    for (k, server) in servers.iter().enumerate() {
        let y = top + 2.0 - scroll.amount.trunc() + k as f32 * ROW_HEIGHT;
        let (cx, cy) = (left + 2.0, y + 2.0);
        let Some(place) = m::place(&server.ip) else {
            continue;
        };
        let m::Place::Country(code) = &place else {
            continue;
        };
        let name_w = HudBatch::text_width(atlas, &server.name);
        let [fx, fy] = m::flag_at(settings.get(m::o::PLACE), cx, cy, name_w, room);
        if fy - 1.0 < top || fy + m::FLAG_H + 1.0 > bottom {
            continue;
        }
        let width = match m::flag(code) {
            Some(cells) => {
                batch.fill(atlas, fx - 1.0, fy - 1.0, m::FLAG_W + 2.0, m::FLAG_H + 2.0, rgba(0, 0, 0, 160));
                for (x, y, w, h, [r, g, b]) in cells {
                    batch.fill(
                        atlas,
                        fx + f32::from(x),
                        fy + f32::from(y),
                        f32::from(w),
                        f32::from(h),
                        rgba(r, g, b, 255),
                    );
                }
                m::FLAG_W
            }
            None if settings.on(m::o::CODES) => {
                let tw = HudBatch::text_width(atlas, code);
                batch.fill(atlas, fx - 1.0, fy - 1.0, tw + 3.0, m::FLAG_H + 2.0, rgba(0, 0, 0, 170));
                batch.text(atlas, code, fx + 0.5, fy, u32::MAX, false);
                tw + 1.0
            }
            None => continue,
        };
        let (px, py) = cursor;
        if px >= fx - 1.0 && px < fx + width + 1.0 && py >= fy - 1.0 && py < fy + m::FLAG_H + 1.0 {
            hovered = m::tooltip(&place);
        }
    }
    if settings.on(m::o::TOOLTIP)
        && let Some(line) = hovered
    {
        crate::ui::tooltip(batch, atlas, &[Text::literal(line.as_str())], cursor);
    }
}
