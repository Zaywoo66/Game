use tauri::{webview::PageLoadEvent, Manager};

mod session;

fn main() {
    let benchmark = std::env::args_os().any(|argument| argument == "--map-benchmark");
    let builder = tauri::Builder::default().invoke_handler(tauri::generate_handler![
        session::game_snapshot,
        session::game_command,
        session::game_ai_log,
        session::lan_status,
        session::lan_discover,
        session::lan_start,
        session::lan_join,
        session::lan_leave,
        session::lan_takeover
    ]);
    let builder = if benchmark {
        builder
            .setup(|app| {
                let window = app
                    .get_webview_window("main")
                    .ok_or_else(|| std::io::Error::other("map benchmark window is unavailable"))?;
                window.set_fullscreen(true)?;
                Ok(())
            })
            .on_page_load(|webview, page| {
                if page.event() != PageLoadEvent::Finished
                    || page.url().query_pairs().any(|(key, _)| key == "benchmark")
                {
                    return;
                }
                let mut url = page.url().clone();
                url.query_pairs_mut().append_pair("benchmark", "1");
                if let Err(error) = webview.navigate(url) {
                    eprintln!("could not open the map benchmark: {error}");
                }
            })
    } else {
        builder.setup(session::setup)
    };

    builder
        .build(tauri::generate_context!())
        .expect("failed to build Strategy app")
        .run(session::on_app_event);
}
