extern crate gio;
extern crate gtk;

use gio::prelude::*;
use gtk::prelude::*;

use gtk::{Application, ApplicationWindow, Container, GtkWindowExt, Label, ListBox, SearchEntry, Widget};

fn main() {
    let application = Application::new(Some("moe.mmf.anni.anniu"), Default::default())
        .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let glade_src = include_str!("../assets/anniu.glade");
        let builder = gtk::Builder::from_string(glade_src);

        let window: ApplicationWindow = builder.get_object("window").unwrap();
        window.set_application(Some(app));
        window.set_title("Anniu");
        window.set_default_size(1280, 720);

        let title_bar: Widget = builder.get_object("player-appbar").unwrap();
        window.set_titlebar(Some(&title_bar));

        let search: SearchEntry = builder.get_object("search").unwrap();
        search.connect_search_changed(move |e| {
            println!("search: {}", e.get_text().as_str());
        });
        let _container: Container = builder.get_object("main-container").unwrap();

        let list_meta: ListBox = builder.get_object("tab-metadata-inner").unwrap();
        list_meta.insert(&Label::new(Some("Test")), 0);
        list_meta.connect_selected_rows_changed(move |e| {
            let row = e.get_selected_row().unwrap().get_index();
            println!("{}", row);
        });

        window.show_all();
    });

    application.run(&[]);
}
