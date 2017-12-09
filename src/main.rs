#![feature(use_extern_macros)]
#![feature(underscore_lifetimes)]
#![feature(drain_filter)]
#![windows_subsystem = "windows"]
extern crate gtk;

extern crate curl;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

extern crate regex;
extern crate yaml_rust;

use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use gtk::prelude::*;

use curl::easy::{Easy, List};

use yaml_rust::YamlLoader;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Url {
    url: String
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Mention {
    screenName: String,
    // userId: String,
}

// User resource with fields from gitter.im
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct User {
    id: String,
    username: String,
    displayName: String,
    url: String,
    avatarUrlSmall: String,
    avatarUrlMedium: String,
}

// Room resource with fields from gitter.im
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Room {
    id: String,
    name: String,
    topic: String,
    url: String,
    oneToOne: bool,
    mentions: u32,
    // favourite: u32,
    githubType: String,
    lurk: bool
}

// Message with fields from gitter.im
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    id: String,
    text: String,
    html: String,
    sent: String,
    fromUser: User,
    unread: bool,
    readBy: i32,
    urls: Vec<Url>,
    mentions: Vec<Mention>,
    v: i32,
}

// Sends and receives data from Gitter.im API
#[derive(Clone)]
struct MessageHandler {
    current_room_id: String,
    token: String,
}

impl MessageHandler {
    fn new(room_id: &String, token: &String) -> MessageHandler {
        MessageHandler {
            current_room_id: room_id.clone(),
            token: token.clone(),
        }
    }

    /** May be used later to allow changing Yaml file during session
    *
    fn set_token(&mut self, token: String) {
        self.token = token;
    }
    */

    fn set_current_room_id(&mut self, id: String) {
        self.current_room_id = id;
    }

    fn load_messages(&self) -> Vec<Message> {
        let url = format!("https://api.gitter.im/v1/rooms/{}/chatMessages?limit=15", &self.current_room_id);
        get_url::<Message>(&url, &self.token)
    }

    fn send_message(&self, message: String) {
        if message.trim().len() == 0 {
            ()
        }

        let url = format!("https://api.gitter.im/v1/rooms/{}/chatMessages", &self.current_room_id);

        let mut easy = Easy::new();

        let json = String::from("{\"text\": \"") + &message[..] + "\"}";
        let mut json = json.as_bytes();

        easy.url(&url).unwrap();
        easy.post(true).unwrap();
        easy.post_field_size(json.len() as u64).unwrap();

        let mut list = List::new();
        list.append("Content-Type: application/json").unwrap();
        list.append("Accept: application/json").unwrap();
        list.append(&(format!("Authorization: Bearer {}", &self.token))).unwrap();
        easy.http_headers(list).unwrap();

        let mut transfer = easy.transfer();
        transfer.read_function(|buf| {
            Ok(json.read(buf).unwrap_or(0))
        }).unwrap();
        transfer.perform().unwrap();
    }
}

// Stores and parses messages
#[derive(Clone, Debug)]
struct MessageStore {
    messages: Vec<Message>,
    last_new_message_id: String,
}

impl MessageStore {
    pub fn new() -> MessageStore {
        MessageStore {
            messages: vec![],
            last_new_message_id: String::from("")
        }
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) -> () {
        let mut message_copy = messages.clone();
        self.messages = self.transform_messages(messages);

        self.last_new_message_id = match message_copy.pop() {
            Some(message) => message.id,
            None => String::from(""),
        };
    }

    pub fn transform_messages(&self, messages: Vec<Message>) -> Vec<Message> {
        if messages.len() == 0 {
            return self.messages.clone();
        }

        let mut messages: Vec<Message> = messages.clone();

        // Reverse vector to quickly determine which messages have already been received
            // during for loop
        messages.reverse();

        let mut final_vec: Vec<Message> = vec![];

        for message in messages.iter() {
            // Once it encounters the familiar id it will stop adding to the new vector
            if self.last_new_message_id == message.id {
                break
            } else {
                final_vec.push(message.clone());
            }
        }

        // Put vector into correct order for adding to GUI
        final_vec.reverse();

        final_vec
    }
}

#[derive(Clone)]
struct MainWindow {
    builder: gtk::Builder,
    headerbar: gtk::HeaderBar,
    scroll_window: gtk::ScrolledWindow,
    scrollable_box: gtk::Box,
    send_text_button: gtk::Button,
    sidebar: gtk::ListBox,
    sidebar_button: gtk::Button,
    sidebar_revealer: gtk::Revealer,
    text_box: gtk::Entry,
    user: User,
    window: gtk::Window,
    viewport: gtk::Viewport,
}

impl MainWindow {
    fn new(user: &User) -> MainWindow {
        if gtk::init().is_err() {
            println!("Failed to initialize GTK.");
        }

        // First we get the file content.
        let glade_src = include_str!("window.glade");

        // Then we call the Builder call.
        let builder = gtk::Builder::new_from_string(glade_src);
        let window: gtk::Window = builder.get_object("window1").unwrap();

        let button: gtk::Button = builder.get_object("sendTextButton").unwrap();
        let entry: gtk::Entry = builder.get_object("textInputBox").unwrap();
        let headerbar: gtk::HeaderBar = builder.get_object("headerbar").unwrap();
        let scroll_window: gtk::ScrolledWindow = builder.get_object("scroll_window").unwrap();
        let scrollable_box: gtk::Box = builder.get_object("scrollable_box").unwrap();
        let sidebar: gtk::ListBox = builder.get_object("sidebar").unwrap();
        let sidebar_button: gtk::Button = builder.get_object("sidebar_button").unwrap();
        let sidebar_revealer: gtk::Revealer = builder.get_object("sidebar_revealer").unwrap();
        let viewport: gtk::Viewport = builder.get_object("viewport").unwrap();

        MainWindow {
            builder: builder,
            window: window,
            headerbar: headerbar,
            send_text_button: button,
            scroll_window: scroll_window,
            scrollable_box: scrollable_box,
            sidebar: sidebar,
            sidebar_button: sidebar_button,
            sidebar_revealer: sidebar_revealer,
            text_box: entry,
            user: user.clone(),
            viewport: viewport,
        }
    }

    fn create_message_label(&self, message: &Message) -> gtk::Label {
        /* To be used or replaced for Markdown to Pango implementation
        let re_mention = regex::Regex::new("(<span) (data.link.type.+) (data.screen.name.+) (class.+)\">").unwrap();
        let re_rel_link = regex::Regex::new(r"\s(rel.+)\s(target.+)\s(class.+)>").unwrap();
        let re_amp = regex::Regex::new(r"&+").unwrap();
        let re_italic = regex::Regex::new(r"em>").unwrap();

        let mut html = String::from(re_mention.replace(&message.html, "<span foreground=\"#22d3a0\">"));
        html = String::from(re_rel_link.replace(&html[..], "class"));
        html = String::from(re_italic.replace(&html[..], "i>"));
        html = String::from(re_amp.replace(&html[..], "&amp;"));
        */

        // println!("{:?}", html);
        // let text = format!("<span foreground='orange'><b>@{}</b></span>: {}", message.fromUser.username, message.text);

        let text = format!("@{}: {}", message.fromUser.username, message.text);

        // let label = gtk::Label::new(None);
        // label.set_markup(&text);
        let label = gtk::Label::new(&text[..]);

        label.set_line_wrap(true);

        let margin = 5;

        label.set_margin_top(margin);
        // label.set_margin_right(margin);
        label.set_margin_bottom(margin);
        // label.set_margin_left(margin);

        label.set_justify(gtk::Justification::Fill);
        label.set_halign(gtk::Align::Start);

        label
    }

    fn add_messages(&mut self, store: &MessageStore, notification_sender: mpsc::Sender<String>) {

        for message in store.messages.iter() {
            let label = self.create_message_label(message);

            self.scrollable_box.add(&label);
            label.show();

            // Check to send notification to notify thread if message mentions current user
            if message.mentions.len() > 0 && &message.mentions[0].screenName == &self.user.username {
                let notification_body = format!("Message from user {}!", message.fromUser.username);
                notification_sender.send(notification_body).unwrap();
            }
        }
    }

    fn add_rooms(&mut self, rooms: &Vec<Room>, send_id: &mpsc::Sender<String>) {
        for room in rooms.iter() {
            let row = gtk::ListBoxRow::new();

            let gtk_box = gtk::EventBox::new();

            let label = gtk::Label::new(Some(&room.name[..]));

            label.set_justify(gtk::Justification::Fill);
            label.set_halign(gtk::Align::Start);

            let self_clone = self.clone();
            // let row_clone = row.clone();
            let room_clone = room.clone();
            let sender = send_id.clone();

            row.connect_button_press_event(move |_this, button| {
                if button.get_button() == 1 {
                    let id: String = room_clone.id.clone();
                    sender.send(id.clone()).unwrap();

                    // Use this instead of function to make compiler happy about &mut self
                    for label in self_clone.scrollable_box.get_children().iter_mut() {
                        &label.destroy();
                    }

                    // Hide sidebar after choosing new room
                    self_clone.sidebar_revealer.set_reveal_child(false);
                }

                gtk::Inhibit(false)
            });

            /* Meant to allow user to click "enter" key to activate
            let self_clone = self.clone();
            let room_clone = room.clone();
            let sender = send_id.clone();

            row.connect_activate(move |_this| {
                let sender = send_id.clone();
                // println!("{:?}", button);
                println!("Room id is {}", room_clone.id);
                let id: String = room_clone.id.clone();
                sender.send(id.clone());
                self_clone.empty_scrollable_box();
            });
            */


            gtk_box.add(&label);

            row.add(&gtk_box);

            self.sidebar.add(&row);
        }
    }

    // Currently a delay here; does not scroll until next thread loop after message received
    fn scroll_to_bottom(&self) {
        let bottom_of_page = self.scroll_window.get_vadjustment().unwrap().get_upper();
        let adjustment = self.scroll_window.get_vadjustment().unwrap();

        adjustment.set_value(bottom_of_page);

        self.scroll_window.set_vadjustment(&adjustment);
    }

    fn show_all(&self) {
        self.window.show_all();
    }

    fn start(&mut self, message_sender: mpsc::Sender<String>) {
        // Set username in subtitle
        {
            let subtitle = format!("@{}", self.user.username);
            self.headerbar.set_subtitle(&subtitle[..]);
        }

        // Window events
        {
            self.window.show_all();
            self.window.connect_delete_event(|_, _| {
                gtk::main_quit();
                Inhibit(false)
            });
        }

        // Send Button click event
        {
            let self_clone = self.clone();
            let self_clone2 = self.clone();
            let clone_message_sender = message_sender.clone();
            // let button_clone = self.send_text_button.clone();
            self_clone.send_text_button.connect_clicked(move |_| {
                let text = self_clone2.text_box.get_text().unwrap();
                println!("{:?}", text);
                clone_message_sender.send(text).unwrap();

                self_clone2.text_box.set_text("");
            });
        }

        // Enter key send message button event
        {
            let self_clone = self.clone();
            let self_clone2 = self.clone();
            let clone_message_sender = message_sender.clone();
            // let button_clone = self.send_text_button.clone();
            self_clone.text_box.connect_key_press_event(move |_this, key| {
                let enter_key = 65293;

                if enter_key == key.get_keyval() {
                    let text = self_clone2.text_box.get_text().unwrap();
                    clone_message_sender.send(text).unwrap();

                    self_clone2.text_box.set_text("");

                    gtk::Inhibit(true);
                }

                gtk::Inhibit(false)
            });
        }

        // Sidebar reveal button event
        {
            let self_clone = self.clone();
            let self_clone2 = self.clone();
            self_clone.sidebar_button.connect_clicked(move |_this| {
                let sidebar_revealer = &self_clone2.sidebar_revealer;
                if sidebar_revealer.get_child_revealed() {
                    sidebar_revealer.set_reveal_child(false);
                } else {
                    sidebar_revealer.set_reveal_child(true);
                }
            });
        }
    }
}

// Takes url and token to get data from Gitter API
fn get_url<T>(url: &String, token: &String) -> Vec<T>
where T: serde::de::DeserializeOwned + Clone
{
    let mut easy = Easy::new();

    easy.url(&url).unwrap();

    let mut list = List::new();

    list.append("Accept: application/json").unwrap();

    list.append(&(format!("Authorization: Bearer {}", token))).unwrap();

    easy.http_headers(list).unwrap();

    let mut raw_data: Vec<u8> = vec![];
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            &raw_data.extend(new_data.iter());

            Ok(new_data.len())
        }).unwrap();

        transfer.perform().unwrap();
    };

    let json_data: Vec<T> = match serde_json::from_slice(&raw_data[..]) {
        Ok(data) => data,
        Err(e) => {
            println!("ERROR Reading Json for {} -> {}", &url, e);
            let empty: Vec<T> = vec![];
            empty
        },
    };

    json_data
}

// Reads config file found in $HOME/.gitter_gtk/config.yaml or cwd
fn read_config() -> yaml_rust::Yaml {
    let config_path = match std::env::var("HOME") {
        Ok(val) => val + "/.gitter_gtk/config.yaml",
        Err(e) => {
            println!("Error: {}\nVariable \"$HOME\", not found. Looking for config.yaml in './'", e);
            String::from("./config.yaml")
        },
    };

    let error_msg = format!("ERROR: File not found at {}!", &config_path);
    let mut buffer = String::new();

    // Read config file
    {
        match File::open(config_path) {
            Ok(mut _f) => {
                println!("Reading from .gitter_gtk");
                _f.read_to_string(&mut buffer).unwrap();
            },
            Err(_) => match File::open(String::from("./config.yaml")) {
                Ok(mut _f) => {
                    println!("Reading from current dir");
                    _f.read_to_string(&mut buffer).unwrap();
                },
                Err(_) => println!("{}", error_msg),
            },
        };
    }

    let base_config = YamlLoader::load_from_str(&buffer[..]).unwrap();
    let config = &base_config[0];

    config.clone()
}

fn message_thread(message_fetcher: Arc<Mutex<MessageHandler>>, mut message_store: MessageStore, message_sender: mpsc::Sender<MessageStore>) {
    std::thread::spawn(move || {
        loop {
            // Update message store
            // Scope locks message_fetcher and then unlocks after setting messages
            {
                let message_fetcher = message_fetcher.lock().unwrap();
                let new_messages = message_fetcher.load_messages();
                &message_store.set_messages(new_messages);
            }

            // Send data to GTK thread
            {
                let data = (&message_store).clone();
                message_sender.send(data).unwrap();
            }

            // Sleep for 5s before checking server again
            {
                let five_secs = std::time::Duration::from_millis(5000);
                std::thread::sleep(five_secs);
            }
        }
    });
}

fn room_thread(message_fetcher: Arc<Mutex<MessageHandler>>, room_id_receiver: mpsc::Receiver<String>) {
    std::thread::spawn(move || {
        loop {
            match room_id_receiver.recv() {
                Ok(id) => {
                    let mut message_fetcher = message_fetcher.lock().unwrap();
                    println!("Setting room id to {}", id);
                    message_fetcher.set_current_room_id(id);
                },
                Err(e) => println!("ERROR Room Id Receiver -> {}", e),
            };
        }
    });
}

fn outgoing_message_thread(message_fetcher: Arc<Mutex<MessageHandler>>, message_receiver: mpsc::Receiver<String>) {
    std::thread::spawn(move || {
        loop {
            match message_receiver.recv() {
                Ok(msg) => {
                    let message_fetcher = message_fetcher.lock().unwrap();
                    message_fetcher.send_message(msg);
                },
                Err(e) => println!("ERROR Outgoing Message Receiver -> {}", e),
            };
        }
    });
}

fn main() {
    let config = read_config();

    let (tx, rx) = mpsc::channel();
    let (tx_room_id, rx_room_id) = mpsc::channel();
    let (tx_notification, rx_notification) = mpsc::channel();
    let (tx_send_message, rx_send_message) = mpsc::channel();

    let token = String::from(config["token"].as_str().unwrap());

    let message_store = MessageStore::new();

    let user: &User = &get_url::<User>(
        &String::from("https://api.gitter.im/v1/user"),
        &String::from(config["token"].as_str().unwrap())
    )[0];

    let mut rooms = get_url::<Room>(
        &String::from("https://api.gitter.im/v1/rooms"),
        &token
    );

    // Alphabetize first, then split into repos and private chats
    {
        rooms.sort_unstable_by(|a, b| {
            a.name.cmp(&b.name)
        });

        let conversations = rooms.drain_filter(|x| x.oneToOne).collect::<Vec<_>>();

        rooms.extend(conversations);
    }

    // Remove mutability from vector
    let rooms = rooms;

    let message_fetcher: Arc<Mutex<MessageHandler>> = Arc::new(Mutex::new(MessageHandler::new(
        &rooms[0].id,
        &token
    )));

    let mut window = MainWindow::new(user);
    {
        window.add_rooms(&rooms, &tx_room_id);
        window.start(tx_send_message);
    }

    // Start our threads to handle logic and keep GUI thread free
    {
        message_thread(message_fetcher.clone(), message_store, tx.clone());

        room_thread(message_fetcher.clone(), rx_room_id);

        outgoing_message_thread(message_fetcher.clone(), rx_send_message);
    }

    // let mut window_clone = window.clone();
    gtk::timeout_add(5000, move || {
        use mpsc::TryRecvError;

        match rx.try_recv() {
            Ok(store) => {
                window.add_messages(&store, tx_notification.clone());

                window.show_all();
                window.scroll_to_bottom();

                gtk::Continue(true)
            },
            Err(TryRecvError::Disconnected) => gtk::Continue(false),
            Err(TryRecvError::Empty) => gtk::Continue(true),
        }

    });

    gtk::main();
}
